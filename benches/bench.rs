#![feature(test)]
extern crate test;

use std::rc::Rc;

use test::Bencher;
use transcode::json::Iter;
use transcode::metadata::*;
use transcode::proto::Encoder;
use transcode::trans_json_to_proto;

fn get_msg_elem_type() -> Type {
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

fn get_msg_foo_embed_type() -> Type {
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

fn get_msg_foo_type() -> Type {
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

const BENCH_CASE0: &[u8] = b"{}";
const BENCH_CASE1: &[u8] = br#"{"a":"a","b":true,"c":1,"d":{"a":2,"b":"b"},"e":[3,4,5],"f":["f0","f1","f2"],"g":[{"a":6,"s":"s0"},{"a":7,"s":"s1"}]}"#;

fn run_trans_json_to_proto(s: &[u8], ty: &Type) {
    let mut enc = Encoder::new();
    let mut it = Iter::new(s);
    let r = trans_json_to_proto(&mut enc, &mut it, ty);
    assert!(r.is_ok());
}

#[bench]
fn bench_trans_json_to_proto_case0(b: &mut Bencher) {
    let ty = get_msg_foo_type();
    b.iter(|| run_trans_json_to_proto(BENCH_CASE0, &ty));
}

#[bench]
fn bench_trans_json_to_proto_case1(b: &mut Bencher) {
    let ty = get_msg_foo_type();
    b.iter(|| run_trans_json_to_proto(BENCH_CASE1, &ty));
}
