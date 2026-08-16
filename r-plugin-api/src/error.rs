use std::fmt;

#[derive(Debug)]
pub enum PluginError {
    InvalidInput(String),
    Processing(String),
    Encode(String),
    Decode(String),
}

impl fmt::Display for PluginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(message) => {
                write!(f, "invalid input: {message}")
            }

            Self::Processing(message) => {
                write!(f, "processing error: {message}")
            }

            Self::Encode(message) => {
                write!(f, "encode error: {message}")
            }

            Self::Decode(message) => {
                write!(f, "decode error: {message}")
            }
        }
    }
}

impl std::error::Error for PluginError {}