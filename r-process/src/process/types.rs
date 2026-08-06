use std::collections::HashMap;

#[derive(Debug)]
pub struct Message {
    pub topic: String,
    pub partition: i32,
    pub offset: i64,
    pub key: Option<Vec<u8>>,
    pub payload: Option<Vec<u8>>,
    pub headers: HashMap<String, Vec<u8>>,
    pub timestamp: Option<i64>,
}

impl Message {
    pub fn new(
        topic: String,
        partition: i32,
        offset: i64,
        key: Option<Vec<u8>>,
        payload: Option<Vec<u8>>,
        headers: HashMap<String, Vec<u8>>,
        timestamp: Option<i64>,
    ) -> Self {
        Self {
            topic,
            partition,
            offset,
            key,
            payload,
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
