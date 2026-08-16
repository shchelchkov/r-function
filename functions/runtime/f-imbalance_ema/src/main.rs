use f_common::fun;
use run_common::imbalance_ema;
use sonic_rs::{JsonContainerTrait, Value};
use std::io::{self, Read, Write};

fn main() {
    let mut buf = Vec::new();
    io::stdin().read_to_end(&mut buf).expect("read stdin");

    let mut v: Value = fun::from_slice(&mut buf);

    if let Some(values) = v.as_array() {
        for value in values.iter() {
            if let Some(obj) = value.as_object() {
                imbalance_ema::emit_ema(obj);
            }
        }
    } else if let Some(obj) = v.as_object() {
        imbalance_ema::emit_ema(obj);
    }

    let out = fun::to_vec(&mut v);
    if let Err(e) = io::stdout().write_all(&out) {
        eprintln!("write stdout failed ({} bytes): {e}", out.len());
        std::process::exit(1);
    }
}
