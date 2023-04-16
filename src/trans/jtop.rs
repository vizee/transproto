use base64::prelude::*;

use crate::json::*;
use crate::metadata::*;
use crate::proto::*;

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
fn trans_numeric(enc: &mut Encoder, kind: &Kind, tag: u32, s: &[u8]) -> Result<()> {
    ::std::str::from_utf8(s)
        .map_err(|e| Error::Wrap(e.into()))
        .and_then(|s| match kind {
            Kind::Double => write_num_field!(enc, tag, s, f64),
            Kind::Float => write_num_field!(enc, tag, s, f32),
            Kind::Int32 => write_num_field!(enc, tag, s, i32, false),
            Kind::Int64 => write_num_field!(enc, tag, s, i64, false),
            Kind::Uint32 => write_num_field!(enc, tag, s, u32, false),
            Kind::Uint64 => write_num_field!(enc, tag, s, u64, false),
            Kind::Sint32 => write_num_field!(enc, tag, s, i32, true),
            Kind::Sint64 => write_num_field!(enc, tag, s, i64, true),
            Kind::Fixed32 => write_num_field!(enc, tag, s, u32),
            Kind::Fixed64 => write_num_field!(enc, tag, s, u64),
            Kind::Sfixed32 => write_num_field!(enc, tag, s, i32),
            Kind::Sfixed64 => write_num_field!(enc, tag, s, i64),
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
    BASE64_STANDARD
        .decode_slice(&s[1..s.len() - 1], &mut z)
        .map_err(|e| Error::Wrap(e.into()))?;
    if !z.is_empty() {
        enc.emit_len_delim(tag, &z);
    }
    Ok(())
}

fn trans_map(enc: &mut Encoder, it: &mut Iter, tag: u32, entry: &Message) -> Result<()> {
    assert_eq!(entry.get_fields().len(), 2);
    let key_field = &entry.get_fields()[0];
    let val_field = &entry.get_fields()[1];
    let mut sub_enc = Encoder::new();
    let mut key: Option<Token> = None;
    while let Some(tok) = it.next() {
        match tok {
            Token::ObjectClose if key.is_none() => return Ok(()),
            Token::Comma | Token::Colon => continue,
            _ => {
                if let Some(k) = key {
                    sub_enc.clear();
                    trans_field(&mut sub_enc, it, 1, k, key_field)?;
                    trans_field(&mut sub_enc, it, 2, tok, val_field)?;
                    let data = sub_enc.as_bytes();
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

fn trans_repeated(enc: &mut Encoder, it: &mut Iter, tag: u32, elem: &Field) -> Result<()> {
    match elem.kind {
        Kind::Message(ref msg) => {
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
        Kind::String => {
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
        Kind::Bytes => {
            let mut z = Vec::new();
            trans_repeated_impl(it, |_, tok| match tok {
                Token::String(s) => {
                    z.clear();
                    BASE64_STANDARD
                        .decode_slice(&s[1..s.len() - 1], &mut z)
                        .map_err(|e| Error::Wrap(e.into()))?;
                    enc.emit_len_delim(tag, &z);
                    Ok(())
                }
                _ => Err(Error::UnexpectedToken),
            })
        }
        _ => {
            let mut packed = Encoder::new();
            match elem.kind {
                Kind::Bool => trans_repeated_impl(it, write_elem_fn!(packed, bool)),
                Kind::Double => trans_repeated_impl(it, write_elem_fn!(packed, f64)),
                Kind::Float => trans_repeated_impl(it, write_elem_fn!(packed, f32)),
                Kind::Int32 => trans_repeated_impl(it, write_elem_fn!(packed, i32, false)),
                Kind::Int64 => trans_repeated_impl(it, write_elem_fn!(packed, i64, false)),
                Kind::Uint32 => trans_repeated_impl(it, write_elem_fn!(packed, u32, false)),
                Kind::Uint64 => trans_repeated_impl(it, write_elem_fn!(packed, u64, false)),
                Kind::Sint32 => trans_repeated_impl(it, write_elem_fn!(packed, i32, true)),
                Kind::Sint64 => trans_repeated_impl(it, write_elem_fn!(packed, i64, true)),
                Kind::Fixed32 => trans_repeated_impl(it, write_elem_fn!(packed, u32)),
                Kind::Fixed64 => trans_repeated_impl(it, write_elem_fn!(packed, u64)),
                Kind::Sfixed32 => trans_repeated_impl(it, write_elem_fn!(packed, i32)),
                Kind::Sfixed64 => trans_repeated_impl(it, write_elem_fn!(packed, i64)),
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

fn trans_field(
    enc: &mut Encoder,
    it: &mut Iter,
    tag: u32,
    lead: Token,
    field: &Field,
) -> Result<()> {
    match lead {
        Token::String(s) => match field.kind {
            Kind::String => trans_string(enc, s, tag),
            Kind::Bytes => trans_bytes(enc, s, tag),
            _ => Err(Error::TypeMismatch),
        },
        Token::Number(n) => trans_numeric(enc, &field.kind, tag, n),
        Token::True | Token::False => match field.kind {
            Kind::Bool => {
                if matches!(lead, Token::True) {
                    enc.emit_varint(tag, 1);
                }
                Ok(())
            }
            _ => Err(Error::TypeMismatch),
        },
        Token::Null => {
            if field.repeated || matches!(field.kind, Kind::Bytes | Kind::Message(_) | Kind::Map(_))
            {
                Ok(())
            } else {
                Err(Error::TypeMismatch)
            }
        }
        Token::Object => match field.kind {
            Kind::Message(ref msg) => trans_embedded_message(enc, it, tag, msg),
            Kind::Map(ref entry) => trans_map(enc, it, tag, entry),
            _ => Err(Error::TypeMismatch),
        },
        Token::Array => {
            if field.repeated {
                trans_repeated(enc, it, tag, field)
            } else {
                Err(Error::TypeMismatch)
            }
        }
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
                        trans_field(enc, it, field.tag, tok, &field)?;
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

pub fn trans_json_to_proto(enc: &mut Encoder, it: &mut Iter, msg: &Message) -> Result<()> {
    match it.next() {
        Some(Token::Object) => trans_message(enc, it, msg),
        None => Err(Error::UnexpectedEof),
        _ => Err(Error::UnexpectedToken),
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
