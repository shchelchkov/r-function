// #[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Command {
    inner: String,
}

impl Command {
    pub fn new(command: &str) -> Self {
        Self {
            inner: command.to_string(),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.inner
    }




    pub fn is_json(&self) -> bool {
        self.as_str() == "json"
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}
