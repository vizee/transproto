#![feature(test)]
extern crate test;

use std::rc::Rc;

use test::Bencher;
use transproto::json::Iter;
use transproto::metadata::*;
use transproto::proto::{Decoder, Encoder};
use transproto::{trans_json_to_proto, trans_proto_to_json};

fn get_msg_elem_type() -> Message {
    Message::new(
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
    )
}

fn get_msg_foo_embed_type() -> Message {
    Message::new(
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
    )
}

fn get_msg_foo_type() -> Message {
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
                kind: Kind::Message(Rc::new(get_msg_foo_embed_type())),
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
                kind: Kind::Message(Rc::new(get_msg_elem_type())),
                repeated: true,
            },
        ],
        true,
    )
}

const BENCH_JSON_CASE0: &[u8] = b"{}";
const BENCH_JSON_CASE1: &[u8] = br#"{"a":"a","b":true,"c":1,"d":{"a":2,"b":"b"},"e":[3,4,5],"f":["f0","f1","f2"],"g":[{"a":6,"s":"s0"},{"a":7,"s":"s1"}]}"#;
const BENCH_PB_CASE0: &[u8] = &[];
const BENCH_PB_CASE1: &[u8] = &[
    10, 1, 97, 16, 1, 24, 1, 34, 5, 8, 2, 18, 1, 98, 42, 3, 3, 4, 5, 50, 2, 102, 48, 50, 2, 102,
    49, 50, 2, 102, 50, 58, 6, 8, 6, 18, 2, 115, 48, 58, 6, 8, 7, 18, 2, 115, 49,
];

fn run_trans_json_to_proto(s: &[u8], msg: &Message) {
    let mut enc = Encoder::new();
    let mut it = Iter::new(s);
    let r = trans_json_to_proto(&mut enc, &mut it, msg);
    assert!(r.is_ok());
}

#[bench]
fn bench_trans_json_to_proto_case0(b: &mut Bencher) {
    let msg = get_msg_foo_type();
    b.iter(|| run_trans_json_to_proto(BENCH_JSON_CASE0, &msg));
}

#[bench]
fn bench_trans_json_to_proto_case1(b: &mut Bencher) {
    let msg = get_msg_foo_type();
    b.iter(|| run_trans_json_to_proto(BENCH_JSON_CASE1, &msg));
}

fn run_trans_proto_to_json(s: &[u8], msg: &Message) {
    let mut buf = Vec::new();
    let mut dec = Decoder::new(s);
    let r = trans_proto_to_json(&mut buf, &mut dec, msg);
    assert!(r.is_ok());
}

#[bench]
fn bench_trans_proto_to_json_case0(b: &mut Bencher) {
    let msg = get_msg_foo_type();
    b.iter(|| run_trans_proto_to_json(BENCH_PB_CASE0, &msg));
}

#[bench]
fn bench_trans_proto_to_json_case1(b: &mut Bencher) {
    let msg = get_msg_foo_type();
    b.iter(|| run_trans_proto_to_json(BENCH_PB_CASE1, &msg));
}
