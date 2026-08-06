use crate::{fun, host, message::convert_value};
use sonic_rs::Object;

const SETTING_CODE: &str = "directory_value";

pub fn value_process(obj: &Object) {
    let topic = fun::f_topic(obj);

    let mut envelope = Object::with_capacity(3);
    envelope.insert("setting_code", SETTING_CODE);
    envelope.insert("key", topic);

    if let Some(values) = convert_value(obj, topic) {
        envelope.insert("value", values);

        let out = match sonic_rs::to_vec(&envelope) {
            Ok(out) => out,
            Err(e) => {
                eprintln!("json_process:::::::::::::: сериализация не удалась: {e}");
                return;
            }
        };

        if let Err(rc) = host::send_value(&out) {
            eprintln!("json_process: send_value завершился с ошибкой: rc={rc}");
        }
    }
}
