use base64::prelude::*;

use crate::json;
use crate::metadata::*;
use crate::proto::*;

use super::append::Append;
use super::*;

macro_rules! write_primitive {
    ($b: expr, $v: expr) => {{
        ($v).append_into($b);
        Ok(())
    }};
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

pub(self) fn field_wire_type(field: &Field) -> u32 {
    if field.repeated {
        WIRE_LEN_DELIM
    } else {
        match field.kind {
            Kind::Double => WIRE_64BIT,
            Kind::Float => WIRE_32BIT,
            Kind::Int32 => WIRE_VARINT,
            Kind::Int64 => WIRE_VARINT,
            Kind::Uint32 => WIRE_VARINT,
            Kind::Uint64 => WIRE_VARINT,
            Kind::Sint32 => WIRE_VARINT,
            Kind::Sint64 => WIRE_VARINT,
            Kind::Fixed32 => WIRE_32BIT,
            Kind::Fixed64 => WIRE_64BIT,
            Kind::Sfixed32 => WIRE_32BIT,
            Kind::Sfixed64 => WIRE_64BIT,
            Kind::Bool => WIRE_VARINT,
            Kind::String => WIRE_LEN_DELIM,
            Kind::Bytes => WIRE_LEN_DELIM,
            Kind::Map(_) => WIRE_LEN_DELIM,
            Kind::Message(_) => WIRE_LEN_DELIM,
        }
    }
}

fn trans_default_value(buf: &mut Vec<u8>, field: &Field) {
    if field.repeated {
        buf.extend_from_slice(b"null")
    } else {
        match field.kind {
            Kind::Double
            | Kind::Float
            | Kind::Int32
            | Kind::Int64
            | Kind::Uint32
            | Kind::Uint64
            | Kind::Sint32
            | Kind::Sint64
            | Kind::Fixed32
            | Kind::Fixed64
            | Kind::Sfixed32
            | Kind::Sfixed64 => buf.push(b'0'),
            Kind::Bool => buf.extend_from_slice(b"false"),
            Kind::String | Kind::Bytes => buf.extend_from_slice(b"\"\""),
            Kind::Map(_) | Kind::Message(_) => buf.extend_from_slice(b"null"),
        }
    }
}

fn trans_map_kv(buf: &mut Vec<u8>, dec: &mut Decoder, entry: &Message) -> Result<()> {
    assert_eq!(entry.get_fields().len(), 2);
    let k_field = &entry.get_fields()[0];
    if !matches!(k_field.kind, Kind::String) {
        return Err(Error::Wrap("key type must be string".into()));
    }
    let v_field = &entry.get_fields()[1];
    let v_wire = field_wire_type(v_field);
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
        trans_string(buf, k)?;
    } else {
        buf.extend_from_slice(b"\"\"");
    }
    buf.push(b':');
    if let Value::None = v_val {
        trans_default_value(buf, v_field);
        Ok(())
    } else {
        trans_field_value(buf, v_field, v_val)
    }
}

fn trans_repeated_impl<T, R>(buf: &mut Vec<u8>, dec: &mut Decoder, r: R) -> Result<()>
where
    T: Append,
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
        r(dec).map(|v| v.append_into(buf)).map_err(Error::from)?;
    }
    buf.push(b']');
    Ok(())
}

fn trans_repeated_field(buf: &mut Vec<u8>, dec: &mut Decoder, field: &Field) -> Result<()> {
    match field.kind {
        Kind::Double => trans_repeated_impl(buf, dec, |dec| dec.read_fixed::<f64>()),
        Kind::Float => trans_repeated_impl(buf, dec, |dec| dec.read_fixed::<f32>()),
        Kind::Int32 => trans_repeated_impl(buf, dec, |dec| dec.read_varint().map(|v| v as i32)),
        Kind::Int64 => trans_repeated_impl(buf, dec, |dec| dec.read_varint().map(|v| v as i64)),
        Kind::Uint32 | Kind::Uint64 => trans_repeated_impl(buf, dec, |dec| dec.read_varint()),
        Kind::Sint32 | Kind::Sint64 => trans_repeated_impl(buf, dec, |dec| dec.read_zigzag()),
        Kind::Fixed32 => trans_repeated_impl(buf, dec, |dec| dec.read_fixed::<u32>()),
        Kind::Fixed64 => trans_repeated_impl(buf, dec, |dec| dec.read_fixed::<u64>()),
        Kind::Sfixed32 => trans_repeated_impl(buf, dec, |dec| dec.read_fixed::<i32>()),
        Kind::Sfixed64 => trans_repeated_impl(buf, dec, |dec| dec.read_fixed::<i64>()),
        Kind::Bool => trans_repeated_impl(buf, dec, |dec| dec.read_varint().map(|v| v != 0)),

        _ => Err(Error::Wrap("unexpected type".into())),
    }
}

fn trans_string(buf: &mut Vec<u8>, data: &[u8]) -> Result<()> {
    buf.push(b'"');
    json::escape_string(data, buf);
    buf.push(b'"');
    Ok(())
}

fn trans_bytes(buf: &mut Vec<u8>, data: &[u8]) -> Result<()> {
    buf.push(b'"');
    let enc_len = (data.len() + 2) / 3 * 4;
    buf.reserve(enc_len);
    let n = buf.len();
    let z = unsafe { std::slice::from_raw_parts_mut(buf.as_mut_ptr().offset(n as isize), enc_len) };
    let m = BASE64_STANDARD
        .encode_slice(data, z)
        .map_err(|e| Error::Wrap(e.into()))?;
    assert_eq!(enc_len, m);
    unsafe { buf.set_len(n + m) };
    buf.push(b'"');
    Ok(())
}

fn trans_field_value(buf: &mut Vec<u8>, field: &Field, v: Value) -> Result<()> {
    if field.repeated {
        trans_repeated_field(buf, &mut Decoder::new(v.into_bytes()), field)
    } else {
        match field.kind {
            Kind::Map(ref entry) => trans_map_kv(buf, &mut Decoder::new(v.into_bytes()), &entry),
            Kind::String => trans_string(buf, v.into_bytes()),
            Kind::Bytes => trans_bytes(buf, v.into_bytes()),
            Kind::Message(ref msg) => trans_message(buf, &mut Decoder::new(v.into_bytes()), msg),
            Kind::Double => write_primitive!(buf, f64::from_le_bytes(v.into_u64().to_le_bytes())),
            Kind::Float => write_primitive!(buf, f32::from_le_bytes(v.into_u32().to_le_bytes())),
            Kind::Int32 => write_primitive!(buf, v.into_u64() as i32),
            Kind::Int64 | Kind::Sfixed64 => write_primitive!(buf, v.into_u64() as i64),
            Kind::Uint32 | Kind::Uint64 | Kind::Fixed64 => write_primitive!(buf, v.into_u64()),
            Kind::Sint32 | Kind::Sint64 => write_primitive!(buf, unzigzag(v.into_u64())),
            Kind::Fixed32 => write_primitive!(buf, v.into_u32()),
            Kind::Sfixed32 => write_primitive!(buf, v.into_u32() as i32),
            Kind::Bool => write_primitive!(buf, v.into_u64() != 0),
        }
    }
}

fn trans_message(buf: &mut Vec<u8>, dec: &mut Decoder, msg: &Message) -> Result<()> {
    let mut cur_tag = 0u32;
    let mut cur_field = None;
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
            cur_field = Some(field);
            expect_wire = field_wire_type(field);

            if !more {
                more = true;
            } else {
                buf.push(b',');
            }
            buf.push(b'"');
            buf.extend_from_slice(field.name.as_bytes());
            buf.push(b'"');
            buf.push(b':');

            if field.repeated {
                match field.kind {
                    Kind::String | Kind::Bytes | Kind::Message(_) => {
                        buf.push(b'[');
                        rep_close = b']';
                        more = false;
                    }
                    _ => {}
                }
            } else if matches!(field.kind, Kind::Map(_)) {
                buf.push(b'{');
                rep_close = b'}';
                more = false;
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
        trans_field_value(buf, cur_field.unwrap(), val)?;
    }

    if rep_close != 0 {
        buf.push(rep_close);
    }
    buf.push(b'}');

    Ok(())
}

pub fn trans_proto_to_json(buf: &mut Vec<u8>, dec: &mut Decoder, msg: &Message) -> Result<()> {
    trans_message(buf, dec, msg)
}

#[cfg(test)]
mod tests {
    use super::super::tests::*;
    use super::*;

    fn test_trans_proto_to_json(s: &[u8], msg: &Message) {
        println!("input: {}", printable(s));
        let mut buf = Vec::new();
        let mut dec = Decoder::new(s);
        let r = trans_proto_to_json(&mut buf, &mut dec, msg);
        if let Err(ref e) = r {
            println!("err: {}", e);
        }
        assert!(r.is_ok());
        println!("output: {}", ::std::str::from_utf8(&buf).unwrap());
    }

    #[test]
    fn test_trans_proto_to_json_case0() {
        test_trans_proto_to_json(&[], &get_msg_foo_type());
    }

    #[test]
    fn test_trans_proto_to_json_case1() {
        test_trans_proto_to_json(
            &[
                34, 0, 42, 3, 0, 0, 0, 50, 0, 50, 0, 50, 0, 58, 0, 58, 0, 58, 0, 58, 0, 58, 0, 58,
                0, 58, 0, 58, 0,
            ],
            &get_msg_foo_type(),
        );
    }

    #[test]
    fn test_trans_proto_to_json_case2() {
        test_trans_proto_to_json(
            &[
                10, 1, 97, 16, 1, 24, 1, 34, 5, 8, 2, 18, 1, 98, 42, 3, 3, 4, 5, 50, 2, 102, 48,
                50, 2, 102, 49, 50, 2, 102, 50, 58, 6, 8, 6, 18, 2, 115, 48, 58, 6, 8, 7, 18, 2,
                115, 49,
            ],
            &get_msg_foo_type(),
        );
    }

    #[test]
    fn test_trans_proto_to_json_case3() {
        test_trans_proto_to_json(
            &[10, 5, 104, 101, 108, 108, 111],
            &Message::new(
                "pbmsg.RawData".to_string(),
                vec![Field {
                    name: "a".to_string(),
                    tag: 1,
                    kind: Kind::Bytes,
                    repeated: false,
                }],
                false,
            ),
        );
    }
}
