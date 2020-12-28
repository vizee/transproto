use std::error;
use std::fmt;
use std::io;
use std::result;

use crate::metadata::*;
use crate::proto::*;

mod jtop;
mod ptoj;

pub use jtop::trans_json_to_proto;
pub use ptoj::trans_proto_to_json;

#[derive(Debug)]
pub enum Error {
    UnexpectedEof,
    UnexpectedToken,
    TypeMismatch,
    InvalidWireType,
    Io(io::Error),
    Wrap(Box<dyn error::Error>),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self {
            Error::UnexpectedEof => f.write_str("unexpected eof"),
            Error::UnexpectedToken => f.write_str("unexpected token"),
            Error::TypeMismatch => f.write_str("type mismatch"),
            Error::InvalidWireType => f.write_str("invalid wire-type"),
            Error::Io(e) => write!(f, "io: {}", e),
            Error::Wrap(e) => write!(f, "wrap: {}", e),
        }
    }
}

impl error::Error for Error {}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

pub type Result<T> = result::Result<T, Error>;

pub(self) fn wire_type(ty: &Type) -> u32 {
    match ty {
        Type::Double => WIRE_64BIT,
        Type::Float => WIRE_32BIT,
        Type::Int32 => WIRE_VARINT,
        Type::Int64 => WIRE_VARINT,
        Type::Uint32 => WIRE_VARINT,
        Type::Uint64 => WIRE_VARINT,
        Type::Sint32 => WIRE_VARINT,
        Type::Sint64 => WIRE_VARINT,
        Type::Fixed32 => WIRE_32BIT,
        Type::Fixed64 => WIRE_64BIT,
        Type::Sfixed32 => WIRE_32BIT,
        Type::Sfixed64 => WIRE_64BIT,
        Type::Bool => WIRE_VARINT,
        Type::String => WIRE_LEN_DELIM,
        Type::Bytes => WIRE_LEN_DELIM,
        Type::Array(_) => WIRE_LEN_DELIM,
        Type::Map(_, _) => WIRE_LEN_DELIM,
        Type::Message(_) => WIRE_LEN_DELIM,
    }
}

#[cfg(test)]
pub mod tests {
    use std::rc::Rc;

    use super::*;

    pub fn printable(s: &[u8]) -> String {
        s.iter()
            .map(|&v| v.to_string())
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn get_msg_elem_type() -> Type {
        Type::Message(Message::new(
            "pbmsg.Elem".to_string(),
            vec![
                Field {
                    name: "a".to_string(),
                    tag: 1,
                    ty: Rc::new(Type::Int32),
                },
                Field {
                    name: "s".to_string(),
                    tag: 2,
                    ty: Rc::new(Type::String),
                },
            ],
            true,
        ))
    }

    pub fn get_msg_foo_embed_type() -> Type {
        Type::Message(Message::new(
            "pbmsg.Foo.Embed".to_string(),
            vec![
                Field {
                    name: "a".to_string(),
                    tag: 1,
                    ty: Rc::new(Type::Int32),
                },
                Field {
                    name: "b".to_string(),
                    tag: 2,
                    ty: Rc::new(Type::String),
                },
            ],
            true,
        ))
    }

    pub fn get_msg_foo_type() -> Type {
        Type::Message(Message::new(
            "pbmsg.Foo".to_string(),
            vec![
                Field {
                    name: "a".to_string(),
                    tag: 1,
                    ty: Rc::new(Type::String),
                },
                Field {
                    name: "b".to_string(),
                    tag: 2,
                    ty: Rc::new(Type::Bool),
                },
                Field {
                    name: "c".to_string(),
                    tag: 3,
                    ty: Rc::new(Type::Int32),
                },
                Field {
                    name: "d".to_string(),
                    tag: 4,
                    ty: Rc::new(get_msg_foo_embed_type()),
                },
                Field {
                    name: "e".to_string(),
                    tag: 5,
                    ty: Rc::new(Type::Array(Rc::new(Type::Int32))),
                },
                Field {
                    name: "f".to_string(),
                    tag: 6,
                    ty: Rc::new(Type::Array(Rc::new(Type::String))),
                },
                Field {
                    name: "g".to_string(),
                    tag: 7,
                    ty: Rc::new(Type::Array(Rc::new(get_msg_elem_type()))),
                },
            ],
            true,
        ))
    }
}
