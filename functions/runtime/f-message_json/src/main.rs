use run_common::message_json;
use sonic_rs::{JsonContainerTrait, Value};
use std::io::{self, Read, Write};
use f_common::fun;

fn main() {
    let mut buf = Vec::new();
    io::stdin().read_to_end(&mut buf).expect("read stdin");

    let mut v: Value = fun::from_slice(&buf);

    if let Some(values) = v.as_array() {
        for value in values.iter() {
            if let Some(obj) = value.as_object() {
                message_json::message_process(obj);
            }
        }
    } else if let Some(obj) = v.as_object() {
        message_json::message_process(obj);
    }

    let out = fun::to_vec(&mut v);
    if let Err(e) = io::stdout().write_all(&out) {
        eprintln!("write stdout failed ({} bytes): {e}", out.len());
        std::process::exit(1);
    }
}
