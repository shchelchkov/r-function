use std::io;
use std::io::{Read, Write};
use sonic_rs::{JsonContainerTrait, Value};
use f_common::signal_obi;

fn main() {
    let mut buf = Vec::new();
    io::stdin().read_to_end(&mut buf).expect("read stdin");

    let v: Value = sonic_rs::from_slice(&buf).expect("parse json");

    if let Some(values) = v.as_array() {
        for value in values.iter() {
            if let Some(obj) = value.as_object() {
                signal_obi::handle(obj);
            }
        }
    } else if let Some(obj) = v.as_object() {
        signal_obi::handle(obj);
    }

    let out = sonic_rs::to_vec(&v).expect("serialize");
    if let Err(e) = io::stdout().write_all(&out) {
        eprintln!("write stdout failed ({} bytes): {e}", out.len());
        std::process::exit(1);
    }
}
