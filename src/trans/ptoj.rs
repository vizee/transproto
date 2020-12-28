use std::fmt::Display;
use std::io::Write;
use std::ptr::NonNull;

use base64::display::Base64Display;

use crate::json;

use super::*;

macro_rules! write_primitive {
    ($b: expr, $v: expr) => {
        $b.write_fmt(format_args!("{}", $v)).map_err(Error::Io)
    };
}

enum Value<'a> {
    None,
    U32(u32),
    U64(u64),
    Bytes(&'a [u8]),
}

impl<'a> Value<'a> {
    fn into_u32(self) -> u32 {
        match self {
            Value::U32(v) => v,
            _ => unreachable!(),
        }
    }

    fn into_u64(self) -> u64 {
        match self {
            Value::U64(v) => v,
            _ => unreachable!(),
        }
    }

    fn into_bytes(self) -> &'a [u8] {
        match self {
            Value::Bytes(s) => s,
            _ => unreachable!(),
        }
    }
}

fn trans_default_value(buf: &mut Vec<u8>, ty: &Type) {
    match ty {
        Type::Double
        | Type::Float
        | Type::Int32
        | Type::Int64
        | Type::Uint32
        | Type::Uint64
        | Type::Sint32
        | Type::Sint64
        | Type::Fixed32
        | Type::Fixed64
        | Type::Sfixed32
        | Type::Sfixed64 => buf.push(b'0'),
        Type::Bool => buf.extend_from_slice(b"false"),
        Type::String | Type::Bytes => buf.extend_from_slice(b"\"\""),
        Type::Array(_) | Type::Map(_, _) | Type::Message(_) => buf.extend_from_slice(b"null"),
    }
}

fn trans_map_kv(buf: &mut Vec<u8>, kty: &Type, vty: &Type, dec: &mut Decoder) -> Result<()> {
    if !matches!(kty, Type::String) {
        return Err(Error::Wrap("key type must be string".into()));
    }
    let v_wire = wire_type(vty);
    let mut k_val = Value::None;
    let mut v_val = Value::None;
    while !dec.eof() {
        let (tag, wire) = dec.read_key().map_err(Error::from)?;
        let val = match wire {
            WIRE_VARINT => dec.read_varint().map(Value::U64).map_err(Error::from)?,
            WIRE_32BIT => dec.read_32bit().map(Value::U32).map_err(Error::from)?,
            WIRE_64BIT => dec.read_64bit().map(Value::U64).map_err(Error::from)?,
            WIRE_LEN_DELIM => dec
                .read_data()
                .map(|s| Value::Bytes(unsafe { &*(s as *const _) as &'static _ }))
                .map_err(Error::from)?,
            _ => return Err(Error::InvalidWireType),
        };
        match tag {
            1 => {
                if wire != WIRE_LEN_DELIM {
                    return Err(Error::InvalidWireType);
                }
                k_val = val;
            }
            2 => {
                if wire != v_wire {
                    return Err(Error::InvalidWireType);
                }
                v_val = val;
            }
            _ => {}
        }
    }
    if let Value::Bytes(k) = k_val {
        trans_string(buf, k).expect("never error");
    } else {
        buf.extend_from_slice(b"\"\"");
    }
    buf.push(b':');
    if let Value::None = v_val {
        trans_default_value(buf, vty);
        Ok(())
    } else {
        trans_value(buf, vty, v_val)
    }
}

fn trans_repeated_impl<T, R>(buf: &mut Vec<u8>, dec: &mut Decoder, r: R) -> Result<()>
where
    T: Display,
    R: Fn(&mut Decoder) -> io::Result<T>,
{
    let mut more = false;
    buf.push(b'[');
    while !dec.eof() {
        if !more {
            more = true;
        } else {
            buf.push(b',');
        };
        r(dec)
            .and_then(|v| buf.write_fmt(format_args!("{}", v)))
            .map_err(Error::from)?;
    }
    buf.push(b']');
    Ok(())
}

fn trans_repeated_packed(buf: &mut Vec<u8>, dec: &mut Decoder, ty: &Type) -> Result<()> {
    match ty {
        Type::Double => trans_repeated_impl(buf, dec, |dec| dec.read_fixed::<f64>()),
        Type::Float => trans_repeated_impl(buf, dec, |dec| dec.read_fixed::<f32>()),
        Type::Int32 => trans_repeated_impl(buf, dec, |dec| dec.read_varint().map(|v| v as i32)),
        Type::Int64 => trans_repeated_impl(buf, dec, |dec| dec.read_varint().map(|v| v as i64)),
        Type::Uint32 | Type::Uint64 => trans_repeated_impl(buf, dec, |dec| dec.read_varint()),
        Type::Sint32 | Type::Sint64 => trans_repeated_impl(buf, dec, |dec| dec.read_zigzag()),
        Type::Fixed32 => trans_repeated_impl(buf, dec, |dec| dec.read_fixed::<u32>()),
        Type::Fixed64 => trans_repeated_impl(buf, dec, |dec| dec.read_fixed::<u64>()),
        Type::Sfixed32 => trans_repeated_impl(buf, dec, |dec| dec.read_fixed::<i32>()),
        Type::Sfixed64 => trans_repeated_impl(buf, dec, |dec| dec.read_fixed::<i64>()),
        Type::Bool => trans_repeated_impl(buf, dec, |dec| dec.read_varint().map(|v| v != 0)),
        _ => Err(Error::Wrap("unexpected type".into())),
    }
}

#[allow(clippy::unnecessary_wraps)]
fn trans_string(buf: &mut Vec<u8>, data: &[u8]) -> Result<()> {
    buf.push(b'"');
    json::escape_string(data, buf);
    buf.push(b'"');
    Ok(())
}

#[allow(clippy::unnecessary_wraps)]
fn trans_bytes(buf: &mut Vec<u8>, data: &[u8]) -> Result<()> {
    buf.push(b'"');
    buf.write_fmt(format_args!(
        "{}",
        Base64Display::with_config(data, base64::STANDARD)
    ))
    .expect("must be ok");
    buf.push(b'"');
    Ok(())
}

fn trans_value(buf: &mut Vec<u8>, ty: &Type, v: Value) -> Result<()> {
    match ty {
        Type::Map(kty, vty) => trans_map_kv(buf, &kty, &vty, &mut Decoder::new(v.into_bytes())),
        Type::Array(elem) => trans_repeated_packed(buf, &mut Decoder::new(v.into_bytes()), &elem),
        Type::String => trans_string(buf, v.into_bytes()),
        Type::Bytes => trans_bytes(buf, v.into_bytes()),
        Type::Message(msg) => trans_message(buf, &mut Decoder::new(v.into_bytes()), msg),
        Type::Double => write_primitive!(buf, f64::from_le_bytes(v.into_u64().to_le_bytes())),
        Type::Float => write_primitive!(buf, f32::from_le_bytes(v.into_u32().to_le_bytes())),
        Type::Int32 => write_primitive!(buf, v.into_u64() as i32),
        Type::Int64 | Type::Sfixed64 => write_primitive!(buf, v.into_u64() as i64),
        Type::Uint32 | Type::Uint64 | Type::Fixed64 => write_primitive!(buf, v.into_u64()),
        Type::Sint32 | Type::Sint64 => write_primitive!(buf, unzigzag(v.into_u64())),
        Type::Fixed32 => write_primitive!(buf, v.into_u32()),
        Type::Sfixed32 => write_primitive!(buf, v.into_u32() as i32),
        Type::Bool => write_primitive!(buf, v.into_u64() != 0),
    }
}

fn trans_message(buf: &mut Vec<u8>, dec: &mut Decoder, msg: &Message) -> Result<()> {
    let mut cur_tag = 0u32;
    let mut cur_type: NonNull<Type> = NonNull::dangling();
    let mut more = false;
    let mut expect_wire = 0u32;
    let mut rep_close = 0u8;

    buf.push(b'{');
    while !dec.eof() {
        let (tag, wire) = dec.read_key().map_err(Error::from)?;
        let val = match wire {
            WIRE_VARINT => dec.read_varint().map(Value::U64).map_err(Error::from)?,
            WIRE_32BIT => dec.read_32bit().map(Value::U32).map_err(Error::from)?,
            WIRE_64BIT => dec.read_64bit().map(Value::U64).map_err(Error::from)?,
            WIRE_LEN_DELIM => dec.read_data().map(Value::Bytes).map_err(Error::from)?,
            _ => return Err(Error::InvalidWireType),
        };

        if tag != cur_tag {
            let field = match msg.get_by_tag(tag) {
                Some(f) => f,
                _ => continue,
            };
            if rep_close != 0 {
                buf.push(rep_close);
                rep_close = 0;
                more = true;
            }

            cur_tag = tag;
            cur_type = NonNull::from(field.ty.as_ref());
            expect_wire = wire_type(&field.ty);

            if !more {
                more = true;
            } else {
                buf.push(b',');
            }
            buf.push(b'"');
            buf.extend_from_slice(field.name.as_bytes());
            buf.push(b'"');
            buf.push(b':');

            match field.ty.as_ref() {
                Type::Array(elem) => match elem.as_ref() {
                    Type::String | Type::Bytes | Type::Message(_) => {
                        cur_type = NonNull::from(elem.as_ref());
                        // expect_wire = wire_type(&field.ty);
                        buf.push(b'[');
                        rep_close = b']';
                        more = false;
                    }
                    _ => {}
                },
                Type::Map(_, _) => {
                    buf.push(b'{');
                    rep_close = b'}';
                    more = false;
                }
                _ => {}
            }
        }
        if wire != expect_wire {
            return Err(Error::InvalidWireType);
        }

        if rep_close != 0 {
            if !more {
                more = true;
            } else {
                buf.push(b',');
            }
        }
        trans_value(buf, unsafe { cur_type.as_ref() }, val)?;
    }

    if rep_close != 0 {
        buf.push(rep_close);
    }
    buf.push(b'}');

    Ok(())
}

pub fn trans_proto_to_json(buf: &mut Vec<u8>, dec: &mut Decoder, ty: &Type) -> Result<()> {
    match ty {
        Type::Message(msg) => trans_message(buf, dec, msg),
        _ => Err(Error::TypeMismatch),
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;

    fn test_trans_proto_to_json(s: &[u8]) {
        println!("input: {}", printable(s));
        let mut buf = Vec::new();
        let mut dec = Decoder::new(s);
        let r = trans_proto_to_json(&mut buf, &mut dec, &get_msg_foo_type());
        if let Err(ref e) = r {
            println!("err: {}", e);
        }
        assert!(r.is_ok());
        println!("output: {}", ::std::str::from_utf8(&buf).unwrap());
    }

    #[test]
    fn test_trans_proto_to_json_case0() {
        test_trans_proto_to_json(&[]);
    }

    #[test]
    fn test_trans_proto_to_json_case1() {
        test_trans_proto_to_json(&[
            34, 0, 42, 3, 0, 0, 0, 50, 0, 50, 0, 50, 0, 58, 0, 58, 0, 58, 0, 58, 0, 58, 0, 58, 0,
            58, 0, 58, 0,
        ]);
    }

    #[test]
    fn test_trans_proto_to_json_case2() {
        test_trans_proto_to_json(&[
            10, 1, 97, 16, 1, 24, 1, 34, 5, 8, 2, 18, 1, 98, 42, 3, 3, 4, 5, 50, 2, 102, 48, 50, 2,
            102, 49, 50, 2, 102, 50, 58, 6, 8, 6, 18, 2, 115, 48, 58, 6, 8, 7, 18, 2, 115, 49,
        ]);
    }
}
