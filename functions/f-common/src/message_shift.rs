use crate::{
    fun, host, message,
    value::{self, FnState},
};
use sonic_rs::{JsonContainerTrait, JsonValueTrait, Object, Value, json};
use crate::user_resource::UserResource;

const SETTING_CODE: &str = "telegram_bot_message";
const STATE_CODE: &str = "message_shift";

pub fn message_shift(obj: &Object) {
    let Some((setting_code, symbol, date_time, state)) = message::data_message_shift(obj) else {
        eprintln!("message_shift::::::::::::::: пропущен ");
        eprintln!(
            "message_shift obj = {}",
            sonic_rs::to_string(obj).unwrap_or_default()
        );
        return;
    };

    let user_resource: UserResource = value::load_function_value(setting_code);
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
    base.insert("setting_code", SETTING_CODE);
    base.insert("process", true);
    base.insert("user_menu", user_resource_list);

    let text = format!("[{state}] {symbol} {}", fun::f_dt_seconds(date_time));

    for user_name in &user_resource.user_list {
        let mut value = base.clone();
        value.insert("user_name", user_name.as_str());
        value.insert("text_to_send", text.as_str());

        let mut envelope = Object::with_capacity(3);
        envelope.insert("setting_code", SETTING_CODE);
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
        eprintln!("message_shift::::::::::::::: send_value {:?}", SETTING_CODE);

        if let Err(rc) = host::send_value(&out) {
            eprintln!("message_shift:::::::::::::::: send_value завершился с ошибкой: rc={rc}");
        }
    }
}
