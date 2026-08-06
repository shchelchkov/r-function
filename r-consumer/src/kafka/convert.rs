use r_process::process::Message;
use rdkafka::message::Headers;
use std::collections::HashMap;

pub(crate) fn into_inbound<M: rdkafka::Message>(m: &M) -> Message {
    let mut headers = HashMap::new();

    if let Some(hs) = m.headers() {
        for h in hs.iter() {
            if let Some(v) = h.value {
                headers.insert(h.key.to_string(), v.to_vec());
            }
        }
    }

    Message::new(
        m.topic().to_string(),
        m.partition(),
        m.offset(),
        m.key().map(|k| k.to_vec()),
        m.payload().map(|p| p.to_vec()),
        headers,
        m.timestamp().to_millis(),
    )
}
