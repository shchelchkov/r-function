use r_config::config::KafkaConfig;
use r_error::kafka::KafkaError;
use rdkafka::ClientConfig;
use rdkafka::producer::FutureProducer;

pub(crate) fn build_producer(cfg: &KafkaConfig) -> Result<FutureProducer, KafkaError> {
    let mut client = ClientConfig::new();
    client
        .set("bootstrap.servers", &cfg.bootstrap_servers)
        .set("client.id", &cfg.client_id)
        .set("group.id", &cfg.group_id)
        .set("enable.auto.commit", "false")
        .set("enable.idempotence", "true");

    for (k, v) in &cfg.parameter {
        client.set(k, v);
    }

    Ok(client.create()?)
}
