use sonic_rs::{JsonContainerTrait, JsonValueTrait, Object, Value};
use std::io::{self, Read, Write};
use f_common::imbalance_ema;

fn main() {
    let mut buf = Vec::new();
    io::stdin().read_to_end(&mut buf).expect("read stdin");

    let v: Value = sonic_rs::from_slice(&buf).expect("parse json");

    if let Some(values) = v.as_array() {
        for value in values.iter() {
            if let Some(obj) = value.as_object() {
                imbalance_ema::emit_ema(obj);
            }
        }
    } else if let Some(obj) = v.as_object() {
        imbalance_ema::emit_ema(obj);
    }

    let out = sonic_rs::to_vec(&v).expect("serialize");
    if let Err(e) = io::stdout().write_all(&out) {
        eprintln!("write stdout failed ({} bytes): {e}", out.len());
        std::process::exit(1);
    }
}
