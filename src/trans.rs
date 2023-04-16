use std::error;
use std::fmt;
use std::io;
use std::result;

mod append;
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

#[cfg(test)]
pub mod tests {
    use std::rc::Rc;

    use crate::metadata::{Field, Kind, Message};

    pub fn printable(s: &[u8]) -> String {
        s.iter()
            .map(|&v| v.to_string())
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn get_msg_elem_type() -> Kind {
        Kind::Message(Rc::new(Message::new(
            "pbmsg.Elem".to_string(),
            vec![
                Field {
                    name: "a".to_string(),
                    tag: 1,
                    kind: Kind::Int32,
                    repeated: false,
                },
                Field {
                    name: "s".to_string(),
                    tag: 2,
                    kind: Kind::String,
                    repeated: false,
                },
            ],
            true,
        )))
    }

    pub fn get_msg_foo_embed_type() -> Kind {
        Kind::Message(Rc::new(Message::new(
            "pbmsg.Foo.Embed".to_string(),
            vec![
                Field {
                    name: "a".to_string(),
                    tag: 1,
                    kind: Kind::Int32,
                    repeated: false,
                },
                Field {
                    name: "b".to_string(),
                    tag: 2,
                    kind: Kind::String,
                    repeated: false,
                },
            ],
            true,
        )))
    }

    pub fn get_msg_foo_type() -> Message {
        Message::new(
            "pbmsg.Foo".to_string(),
            vec![
                Field {
                    name: "a".to_string(),
                    tag: 1,
                    kind: Kind::String,
                    repeated: false,
                },
                Field {
                    name: "b".to_string(),
                    tag: 2,
                    kind: Kind::Bool,
                    repeated: false,
                },
                Field {
                    name: "c".to_string(),
                    tag: 3,
                    kind: Kind::Int32,
                    repeated: false,
                },
                Field {
                    name: "d".to_string(),
                    tag: 4,
                    kind: get_msg_foo_embed_type(),
                    repeated: false,
                },
                Field {
                    name: "e".to_string(),
                    tag: 5,
                    kind: Kind::Int32,
                    repeated: true,
                },
                Field {
                    name: "f".to_string(),
                    tag: 6,
                    kind: Kind::String,
                    repeated: true,
                },
                Field {
                    name: "g".to_string(),
                    tag: 7,
                    kind: get_msg_elem_type(),
                    repeated: true,
                },
            ],
            true,
        )
    }
}
