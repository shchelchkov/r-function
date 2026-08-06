use r_producer::kafka::producer::DlqContext;
use r_runtime_api::Runtime;
use std::sync::Arc;

use crate::process::Message;
use crate::process::chain;
use crate::process::grouping::{Group, Grouped, group_messages};
use crate::process::provider::SettingProvider;
use crate::process::publisher::MessagePublisher;
use crate::process::resolver;

pub struct Processor {
    producer: Arc<dyn MessagePublisher>,
    function: Arc<dyn SettingProvider>,
    runtime: Arc<dyn Runtime>,
}

impl Processor {
    pub fn new(
        producer: Arc<dyn MessagePublisher>,
        function: Arc<dyn SettingProvider>,
        runtime: Arc<dyn Runtime>,
    ) -> Self {
        Self {
            producer,
            function,
            runtime,
        }
    }

            pub async fn handle_batch(&self, msgs: Vec<Message>) -> Vec<Result<(), ()>> {
        let resolved = resolver::resolve_all(&self.function, &msgs).await;
        let Grouped {
            mut results,
            groups,
            poison,
        } = group_messages(&msgs, &resolved);
        for (idx, reason) in poison {
            self.route_dlq(&msgs[idx], &reason).await;
        }
        for (setting_code, group) in groups {
            let res = self.emit_group(&msgs, &setting_code, &group).await;
            for &idx in &group.idxs {
                results[idx] = res;
            }
        }
        results
    }

    async fn emit_group(
        &self,
        msgs: &[Message],
        setting_code: &str,
        group: &Group,
    ) -> Result<(), ()> {
        match chain::execute_batch(&self.runtime, &group.batch, &group.settings).await {
            Ok(Some(result)) => {
                match self
                    .producer
                    .send_objects(setting_code, group.out_key.as_deref(), result)
                    .await
                {
                    Ok(()) => Ok(()),
                    Err(e) => {
                        tracing::warn!(error = %e, setting_code, "group send failed (transient)");
                        Err(()) 
                    }
                }
            }
            Ok(None) => Ok(()), 
            Err(e) if e.is_transient() => {
                tracing::warn!(error = %e, setting_code, "group transient failure");
                Err(())
            }
            Err(e) => {
                let reason = e.to_string();
                tracing::warn!(error = %reason, setting_code, "group poison; routing to DLQ");
                for &idx in &group.idxs {
                    self.route_dlq(&msgs[idx], &reason).await;
                }
                Ok(())
            }
        }
    }

    async fn route_dlq(&self, msg: &Message, reason: &str) {
        let Some(raw) = msg.payload.as_deref() else {
            return;
        };
        let ctx = DlqContext {
            reason,
            source_topic: msg.topic(),
            source_partition: msg.partition(),
            source_offset: msg.offset(),
        };
        if let Err(e) = self.producer.send_dlq(raw, msg.key.as_deref(), ctx).await {
            tracing::error!(error = %e, "dlq send failed");
        }
    }
}
