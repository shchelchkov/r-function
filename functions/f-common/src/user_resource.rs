use crate::{
    fun,
    value::FnState,
};
use sonic_rs::{JsonContainerTrait, JsonValueTrait, Object, Value, json};

const STATE_CODE: &str = "user_resource";

#[derive(Default, Clone)]
pub struct UserResource {
    pub text_to_send: String,
    pub text: String,
    pub date_time: String,
    pub chat_id: i64,
    pub user_id: i64,
    pub user_name: String,
    pub command: String,
    pub setting_code: String,
    pub user_resource_list: Vec<String>,
    pub symbol: Vec<String>,
    pub user_list: Vec<String>,
    pub is_active: bool,

}

impl FnState for UserResource {
    const CODE: &'static str = STATE_CODE;

    fn from_value(values: &[Value]) -> Self {
        let Some(obj) = values.first().and_then(|v| v.as_object()) else {
            return Self::default();
        };
        Self {
            text_to_send: fun::f_val(obj, "text_to_send"),
            text: fun::f_val(obj, "text"),
            date_time: fun::f_val(obj, "date_time"),

            chat_id: fun::f_i64(obj, "chat_id"),
            user_id: fun::f_i64(obj, "user_id"),

            user_name: fun::f_val(obj, "user_name"),
            command: fun::f_val(obj, "command"),
            setting_code: fun::f_val(obj, "setting_code"),

            user_resource_list: fun::f_vec(obj, "user_resource_list"),
            symbol: fun::f_vec(obj, "symbol"),
            user_list: fun::f_vec(obj, "user_list"),

            is_active: fun::f_bool(obj, "is_active"),
        }
    }

    fn into_value(self) -> Value {
        let mut o = Object::with_capacity(8);
        o.insert("text_to_send", self.text_to_send.as_str());
        o.insert("text", self.text.as_str());
        o.insert("date_time", self.date_time.as_str());
        o.insert("chat_id", self.chat_id);
        o.insert("user_id", self.user_id);
        o.insert("user_name", self.user_name.as_str());
        o.insert("command", self.command.as_str());
        o.insert("setting_code", self.setting_code.as_str());
        o.insert("user_resource_list", json!(self.user_resource_list));
        o.insert("user_list", json!(self.user_list));
        o.insert("is_active", self.is_active);
        o.into_value()
    }
}