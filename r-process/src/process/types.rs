use std::collections::HashMap;

use value_rs::Value;

#[derive(Debug)]
pub struct Message {
    pub topic: String,
    pub partition: i32,
    pub offset: i64,
    pub key: Option<Vec<u8>>,
    pub raw: Option<Vec<u8>>,
    pub payload: Option<Vec<Value>>,
    pub headers: HashMap<String, Vec<u8>>,
    pub timestamp: Option<i64>,
}

impl Message {
    pub fn new(
        topic: String,
        partition: i32,
        offset: i64,
        key: Option<Vec<u8>>,
        raw: Option<Vec<u8>>,
        headers: HashMap<String, Vec<u8>>,
        timestamp: Option<i64>,
    ) -> Self {
        Self {
            topic,
            partition,
            offset,
            key,
            raw,
            payload: None,
            headers,
            timestamp,
        }
    }

    #[inline]
    pub fn topic(&self) -> &str {
        &self.topic
    }
    #[inline]
    pub fn partition(&self) -> i32 {
        self.partition
    }
    #[inline]
    pub fn offset(&self) -> i64 {
        self.offset
    }
}
