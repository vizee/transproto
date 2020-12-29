#![feature(test)]
extern crate test;

use std::rc::Rc;

use test::Bencher;
use transcode::json::Iter;
use transcode::metadata::*;
use transcode::proto::{Decoder, Encoder};
use transcode::{trans_json_to_proto, trans_proto_to_json};

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

const BENCH_JSON_CASE0: &[u8] = b"{}";
const BENCH_JSON_CASE1: &[u8] = br#"{"a":"a","b":true,"c":1,"d":{"a":2,"b":"b"},"e":[3,4,5],"f":["f0","f1","f2"],"g":[{"a":6,"s":"s0"},{"a":7,"s":"s1"}]}"#;
const BENCH_PB_CASE0: &[u8] = &[];
const BENCH_PB_CASE1: &[u8] = &[
    10, 1, 97, 16, 1, 24, 1, 34, 5, 8, 2, 18, 1, 98, 42, 3, 3, 4, 5, 50, 2, 102, 48, 50, 2, 102,
    49, 50, 2, 102, 50, 58, 6, 8, 6, 18, 2, 115, 48, 58, 6, 8, 7, 18, 2, 115, 49,
];

fn run_trans_json_to_proto(s: &[u8], ty: &Type) {
    let mut enc = Encoder::new();
    let mut it = Iter::new(s);
    let r = trans_json_to_proto(&mut enc, &mut it, ty);
    assert!(r.is_ok());
}

#[bench]
fn bench_trans_json_to_proto_case0(b: &mut Bencher) {
    let ty = get_msg_foo_type();
    b.iter(|| run_trans_json_to_proto(BENCH_JSON_CASE0, &ty));
}

#[bench]
fn bench_trans_json_to_proto_case1(b: &mut Bencher) {
    let ty = get_msg_foo_type();
    b.iter(|| run_trans_json_to_proto(BENCH_JSON_CASE1, &ty));
}

fn run_trans_proto_to_json(s: &[u8], ty: &Type) {
    let mut buf = Vec::new();
    let mut dec = Decoder::new(s);
    let r = trans_proto_to_json(&mut buf, &mut dec, ty);
    assert!(r.is_ok());
}

#[bench]
fn bench_trans_proto_to_json_case0(b: &mut Bencher) {
    let ty = get_msg_foo_type();
    b.iter(|| run_trans_proto_to_json(BENCH_PB_CASE0, &ty));
}

#[bench]
fn bench_trans_proto_to_json_case1(b: &mut Bencher) {
    let ty = get_msg_foo_type();
    b.iter(|| run_trans_proto_to_json(BENCH_PB_CASE1, &ty));
}
