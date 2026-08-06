use serde_json::{Value, json};
use std::io::{self, Read, Write};

fn main() {
    let mut buf = Vec::new();
    io::stdin().read_to_end(&mut buf).expect("read stdin");

    let mut v: Value = serde_json::from_slice(&buf).expect("parse json");

    match &mut v {
        Value::Array(items) => {
            for item in items {
                if let Some(obj) = item.as_object_mut() {
                    obj.insert("processed".into(), json!(true));
                }
            }
        }
        other => {
            if let Some(obj) = other.as_object_mut() {
                obj.insert("processed".into(), json!(true));
            }
        }
    }

    let out = serde_json::to_vec(&v).expect("serialize");
    io::stdout().write_all(&out).expect("write stdout");
}
