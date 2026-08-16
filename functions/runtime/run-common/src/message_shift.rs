use crate::{
    host,
    value::{self},
};
use f_common::{fun, message};

use crate::user_resource::UserResource;
use sonic_rs::{Object, json};

pub fn message_shift(obj: &Object) {
    eprintln!(
        "message_shift::::::::::::: 0001 obj = {}",
        sonic_rs::to_string(obj).unwrap_or_default()
    );

    let Some((setting_code, symbol, date_time, state)) = message::data_message_shift(obj) else {
        eprintln!("message_shift::::::::::::::: пропущен ");
        eprintln!(
            "message_shift obj = {}",
            sonic_rs::to_string(obj).unwrap_or_default()
        );
        return;
    };

    let user_resource: UserResource = usr_res(setting_code);
    let is_active: bool;
    let user_resource_list = if user_resource.user_resource_list.is_empty() {
        is_active = false;
        json!(null)
    } else {
        is_active = true;
        json!(user_resource.user_resource_list)
    };

    if !is_active {
        return;
    }

    if user_resource.user_list.is_empty() {
        return;
    }

    if !user_resource.symbol.is_empty() && !user_resource.symbol.iter().any(|s| s == symbol) {
        return;
    }

    let mut base = Object::with_capacity(obj.len() + 32);
    for (key, val) in obj.iter() {
        base.insert(key, val.clone());
    }
    base.insert("setting_code", user_resource.setting_code.as_str());
    base.insert("process", true);
    base.insert("user_menu", user_resource_list);

    let text = format!("[{state}] {symbol} {}", fun::f_dt_seconds(date_time));

    for user_name in &user_resource.user_list {
        let mut value = base.clone();
        value.insert("user_name", user_name.as_str());
        value.insert("text_to_send", text.as_str());

        let mut envelope = Object::with_capacity(3);
        envelope.insert("setting_code", user_resource.setting_code.as_str());
        envelope.insert("key", fun::f_topic(obj));

        let mut values = Vec::new();
        values.push(value);
        envelope.insert("value", values);

        let out = match sonic_rs::to_vec(&envelope) {
            Ok(out) => out,
            Err(e) => {
                eprintln!("message_shift:::::::::::::: сериализация не удалась: {e}");
                continue;
            }
        };

        if let Err(rc) = host::send_value(&out) {
            eprintln!("message_shift:::::::::::::::: send_value завершился с ошибкой: rc={rc}");
        }
    }
}

fn usr_res(setting_code: &str) -> UserResource {
    eprintln!("message_shift::::::::::::: 0001 setting_code: {setting_code}");
    value::load_function_value(setting_code)
}
