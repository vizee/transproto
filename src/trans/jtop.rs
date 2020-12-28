use std::rc::Rc;

use crate::json::*;

use super::*;

macro_rules! write_num_field {
    ($e: expr, $tag: expr, $s: expr, $ty: ty, $z: expr) => {
        $s.parse::<$ty>()
            .map(|v| {
                if v != 0 as $ty {
                    if $z {
                        $e.emit_zigzag($tag, v as i64);
                    } else {
                        $e.emit_varint($tag, v as u64);
                    }
                }
            })
            .map_err(|e| Error::Wrap(e.into()))
    };
    ($e: expr, $tag: expr, $s: expr, $ty: ty) => {
        $s.parse::<$ty>()
            .map(|v| {
                if v != 0 as $ty {
                    $e.write_varint(proto_key($tag, if ::std::mem::size_of::<$ty>() == 64 {
                        WIRE_64BIT
                    } else {
                        WIRE_32BIT
                    }));
                    $e.write_slice(&v.to_le_bytes()[..]);
                }
            })
            .map_err(|e| Error::Wrap(e.into()))
    };
}

macro_rules! write_elem_fn {
    ($enc: expr, bool) => {
        |_, tok| match tok {
            Token::True | Token::False => {
                $enc.write_varint(matches!(tok, Token::True) as u64);
                Ok(())
            }
            _ => Err(Error::UnexpectedToken),
        }
    };
    ($enc: expr, $ty: ty) => {
        |_, tok| match tok {
            Token::Number(s) => ::std::str::from_utf8(s)
                .map_err(|e| Error::Wrap(e.into()))
                .and_then(|s| {
                    s.parse::<$ty>()
                        .map(|v| $enc.write_slice(&v.to_le_bytes()[..]))
                        .map_err(|e| Error::Wrap(e.into()))
                }),
            _ => Err(Error::UnexpectedToken),
        }
    };
    ($enc: expr, $ty: ty, $z: expr) => {
        |_, tok| match tok {
            Token::Number(s) => ::std::str::from_utf8(s)
                .map_err(|e| Error::Wrap(e.into()))
                .and_then(|s| {
                    s.parse::<$ty>()
                        .map(|v| {
                            if $z {
                                $enc.write_zigzag(v as i64);
                            } else {
                                $enc.write_varint(v as u64);
                            }
                        })
                        .map_err(|e| Error::Wrap(e.into()))
                }),
            _ => Err(Error::UnexpectedToken),
        }
    };
}

fn skip_value(it: &mut Iter, tok: Token) -> Result<()> {
    match tok {
        Token::Null | Token::False | Token::True | Token::Number(_) | Token::String(_) => Ok(()),
        Token::Object => {
            while let Some(tok) = it.next() {
                match tok {
                    Token::ObjectClose => return Ok(()),
                    Token::Colon | Token::Comma => continue,
                    _ => skip_value(it, tok)?,
                }
            }
            Err(Error::UnexpectedEof)
        }
        Token::Array => {
            while let Some(tok) = it.next() {
                match tok {
                    Token::ArrayClose => return Ok(()),
                    Token::Comma => continue,
                    _ => skip_value(it, tok)?,
                }
            }
            Err(Error::UnexpectedEof)
        }
        _ => Err(Error::UnexpectedToken),
    }
}

#[allow(clippy::float_cmp)]
fn trans_numeric(enc: &mut Encoder, ty: &Type, tag: u32, s: &[u8]) -> Result<()> {
    ::std::str::from_utf8(s)
        .map_err(|e| Error::Wrap(e.into()))
        .and_then(|s| match ty {
            Type::Double => write_num_field!(enc, tag, s, f64),
            Type::Float => write_num_field!(enc, tag, s, f32),
            Type::Int32 => write_num_field!(enc, tag, s, i32, false),
            Type::Int64 => write_num_field!(enc, tag, s, i64, false),
            Type::Uint32 => write_num_field!(enc, tag, s, u32, false),
            Type::Uint64 => write_num_field!(enc, tag, s, u64, false),
            Type::Sint32 => write_num_field!(enc, tag, s, i32, true),
            Type::Sint64 => write_num_field!(enc, tag, s, i64, true),
            Type::Fixed32 => write_num_field!(enc, tag, s, u32),
            Type::Fixed64 => write_num_field!(enc, tag, s, u64),
            Type::Sfixed32 => write_num_field!(enc, tag, s, i32),
            Type::Sfixed64 => write_num_field!(enc, tag, s, i64),
            _ => Err(Error::TypeMismatch),
        })
}

fn trans_string(enc: &mut Encoder, s: &[u8], tag: u32) -> Result<()> {
    let mut z = Vec::with_capacity(s.len() - 2);
    unescape_string(&s[1..s.len() - 1], &mut z).map_err(|e| Error::Wrap(e.into()))?;
    if !z.is_empty() {
        enc.emit_len_delim(tag, &z);
    }
    Ok(())
}

fn trans_bytes(enc: &mut Encoder, s: &[u8], tag: u32) -> Result<()> {
    let mut z = Vec::with_capacity(s.len() * 4 / 3);
    base64::decode_config_buf(&s[1..s.len() - 1], base64::STANDARD, &mut z)
        .map_err(|e| Error::Wrap(e.into()))?;
    if !z.is_empty() {
        enc.emit_len_delim(tag, &z);
    }
    Ok(())
}

fn trans_map(enc: &mut Encoder, it: &mut Iter, tag: u32, kty: &Type, vty: &Type) -> Result<()> {
    let mut entry = Encoder::new();
    let mut key: Option<Token> = None;
    while let Some(tok) = it.next() {
        match tok {
            Token::ObjectClose if key.is_none() => return Ok(()),
            Token::Comma | Token::Colon => continue,
            _ => {
                if let Some(k) = key {
                    entry.clear();
                    trans_field(&mut entry, it, 1, k, kty)?;
                    trans_field(&mut entry, it, 2, tok, vty)?;
                    let data = entry.as_bytes();
                    if !data.is_empty() {
                        enc.emit_len_delim(tag, data);
                    }
                    key = None;
                } else if matches!(tok, Token::String(_)) {
                    key = Some(tok);
                } else {
                    return Err(Error::UnexpectedToken);
                }
            }
        }
    }
    Err(Error::UnexpectedEof)
}

fn trans_repeated_impl<F>(it: &mut Iter, mut f: F) -> Result<()>
where
    F: FnMut(&mut Iter, Token) -> Result<()>,
{
    while let Some(tok) = it.next() {
        match tok {
            Token::Comma => continue,
            Token::ArrayClose => return Ok(()),
            _ => f(it, tok)?,
        }
    }
    Err(Error::UnexpectedEof)
}

fn trans_repeated(enc: &mut Encoder, it: &mut Iter, tag: u32, elem: &Rc<Type>) -> Result<()> {
    match elem.as_ref() {
        Type::Message(msg) => {
            let mut z = Encoder::new();
            trans_repeated_impl(it, |it, tok| match tok {
                Token::Object => {
                    z.clear();
                    trans_message(&mut z, it, msg)?;
                    enc.emit_len_delim(tag, z.as_bytes());
                    Ok(())
                }
                _ => Err(Error::UnexpectedToken),
            })
        }
        Type::String => {
            let mut z = Vec::new();
            trans_repeated_impl(it, |_, tok| match tok {
                Token::String(s) => {
                    z.clear();
                    unescape_string(&s[1..s.len() - 1], &mut z)
                        .map_err(|e| Error::Wrap(e.into()))?;
                    enc.emit_len_delim(tag, &z);
                    Ok(())
                }
                _ => Err(Error::UnexpectedToken),
            })
        }
        Type::Bytes => {
            let mut z = Vec::new();
            trans_repeated_impl(it, |_, tok| match tok {
                Token::String(s) => {
                    z.clear();
                    base64::decode_config_buf(&s[1..s.len() - 1], base64::STANDARD, &mut z)
                        .map_err(|e| Error::Wrap(e.into()))?;
                    enc.emit_len_delim(tag, &z);
                    Ok(())
                }
                _ => Err(Error::UnexpectedToken),
            })
        }
        _ => {
            let mut packed = Encoder::new();
            match elem.as_ref() {
                Type::Bool => trans_repeated_impl(it, write_elem_fn!(packed, bool)),
                Type::Double => trans_repeated_impl(it, write_elem_fn!(packed, f64)),
                Type::Float => trans_repeated_impl(it, write_elem_fn!(packed, f32)),
                Type::Int32 => trans_repeated_impl(it, write_elem_fn!(packed, i32, false)),
                Type::Int64 => trans_repeated_impl(it, write_elem_fn!(packed, i64, false)),
                Type::Uint32 => trans_repeated_impl(it, write_elem_fn!(packed, u32, false)),
                Type::Uint64 => trans_repeated_impl(it, write_elem_fn!(packed, u64, false)),
                Type::Sint32 => trans_repeated_impl(it, write_elem_fn!(packed, i32, true)),
                Type::Sint64 => trans_repeated_impl(it, write_elem_fn!(packed, i64, true)),
                Type::Fixed32 => trans_repeated_impl(it, write_elem_fn!(packed, u32)),
                Type::Fixed64 => trans_repeated_impl(it, write_elem_fn!(packed, u64)),
                Type::Sfixed32 => trans_repeated_impl(it, write_elem_fn!(packed, i32)),
                Type::Sfixed64 => trans_repeated_impl(it, write_elem_fn!(packed, i64)),
                _ => return Err(Error::TypeMismatch),
            }?;
            if !packed.is_empty() {
                enc.emit_len_delim(tag, packed.as_bytes());
            }
            Ok(())
        }
    }
}

fn trans_embedded_message(enc: &mut Encoder, it: &mut Iter, tag: u32, msg: &Message) -> Result<()> {
    let mut embedded = Encoder::new();
    trans_message(&mut embedded, it, msg)?;
    enc.emit_len_delim(tag, embedded.as_bytes());
    Ok(())
}

fn trans_field(enc: &mut Encoder, it: &mut Iter, tag: u32, lead: Token, ty: &Type) -> Result<()> {
    match lead {
        Token::String(s) => match ty {
            Type::String => trans_string(enc, s, tag),
            Type::Bytes => trans_bytes(enc, s, tag),
            _ => Err(Error::TypeMismatch),
        },
        Token::Number(n) => trans_numeric(enc, ty, tag, n),
        Token::True | Token::False => match ty {
            Type::Bool => {
                if matches!(lead, Token::True) {
                    enc.emit_varint(tag, 1);
                }
                Ok(())
            }
            _ => Err(Error::TypeMismatch),
        },
        Token::Null => match ty {
            Type::Bytes | Type::Message(_) | Type::Array(_) | Type::Map(_, _) => Ok(()),
            _ => Err(Error::TypeMismatch),
        },
        Token::Object => match ty {
            Type::Message(msg) => trans_embedded_message(enc, it, tag, msg),
            Type::Map(kty, vty) => trans_map(enc, it, tag, kty, vty),
            _ => Err(Error::TypeMismatch),
        },
        Token::Array => match ty {
            Type::Array(elem) => trans_repeated(enc, it, tag, elem),
            _ => Err(Error::TypeMismatch),
        },
        _ => Err(Error::UnexpectedToken),
    }
}

fn trans_message(enc: &mut Encoder, it: &mut Iter, msg: &Message) -> Result<()> {
    let mut key: Option<&[u8]> = None;
    while let Some(tok) = it.next() {
        match tok {
            Token::ObjectClose if key.is_none() => return Ok(()),
            Token::Comma | Token::Colon => continue,
            _ => {
                if let Some(k) = key {
                    let name = ::std::str::from_utf8(&k[1..k.len() - 1])
                        .map_err(|e| Error::Wrap(e.into()))?;
                    if let Some(field) = msg.get_by_name(name) {
                        trans_field(enc, it, field.tag, tok, &field.ty)?;
                    } else {
                        skip_value(it, tok)?;
                    }
                    key = None;
                } else if let Token::String(k) = tok {
                    key = Some(k);
                } else {
                    return Err(Error::UnexpectedToken);
                }
            }
        }
    }
    Err(Error::UnexpectedEof)
}

pub fn trans_json_to_proto(enc: &mut Encoder, it: &mut Iter, ty: &Type) -> Result<()> {
    match ty {
        Type::Message(msg) => match it.next() {
            Some(Token::Object) => trans_message(enc, it, msg),
            None => Err(Error::UnexpectedEof),
            _ => Err(Error::UnexpectedToken),
        },
        _ => Err(Error::TypeMismatch),
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;

    fn test_trans_json_to_proto(s: &str) {
        println!("input: {}", s);
        let mut enc = Encoder::new();
        let mut it = Iter::new(s.as_bytes());
        let r = trans_json_to_proto(&mut enc, &mut it, &get_msg_foo_type());
        if let Err(ref e) = r {
            println!("err: {}", e);
        }
        assert!(r.is_ok());
        println!("output: {}", printable(enc.as_bytes()));
    }

    #[test]
    fn test_trans_json_to_proto_case0() {
        let s = r#"{}"#;
        test_trans_json_to_proto(s);
    }

    #[test]
    fn test_trans_json_to_proto_case1() {
        let s = r#"{"a":"a","b":true,"c":1,"d":{"a":2,"b":"b"},"e":[3,4,5],"f":["f0","f1","f2"],"g":[{"a":6,"s":"s0"},{"a":7,"s":"s1"}]}"#;
        test_trans_json_to_proto(s);
    }

    #[test]
    fn test_trans_json_to_proto_case2() {
        let s = r#"{"a":"","b":false,"c":0,"d":{"a":0,"b":""},"e":[0,0,0],"f":["","",""],"g":[{"a":0,"s":""},{"a":0,"s":""},{"a":0,"s":""},{"a":0,"s":""},{"a":0,"s":""},{"a":0,"s":""},{"a":0,"s":""},{"a":0,"s":""}]}"#;
        test_trans_json_to_proto(s);
    }
}
