use std::fmt;

pub struct Credentials {
    pub(super) api_key: String,
    pub(super) api_secret: String,
}

impl Credentials {
    pub fn new(api_key: impl Into<String>, api_secret: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            api_secret: api_secret.into(),
        }
    }
}

impl fmt::Debug for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Credentials")
            .field("api_key", &self.api_key)
            .field("api_secret", &"***")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_hides_secret() {
        let dbg = format!("{:?}", Credentials::new("testkey123", "testsecret456"));
        assert!(dbg.contains("testkey123"));
        assert!(!dbg.contains("testsecret456"));
    }
}
