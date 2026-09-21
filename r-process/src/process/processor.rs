use crate::process::Message;
use crate::process::chain;
use crate::process::grouping::{Group, Grouped, group_messages};
use crate::process::publisher::MessagePublisher;
use crate::process::resolver::Resolver;
use r_error::process::error::ProcessError;
use r_plugin_api::Plugin;
use r_producer::kafka::producer::DlqContext;
use r_runtime_api::Runtime;
use std::sync::Arc;

pub struct Processor {
    pub producer: Arc<dyn MessagePublisher>,
    pub resolver: Arc<Resolver>,
    pub runtime: Arc<dyn Runtime>,
    pub plugin: Arc<dyn Plugin>,
}

impl Processor {
    pub fn new(
        producer: Arc<dyn MessagePublisher>,
        resolver: Arc<Resolver>,
        runtime: Arc<dyn Runtime>,
        plugin: Arc<dyn Plugin>,
    ) -> Self {
        Self {
            producer,
            resolver,
            runtime,
            plugin,
        }
    }

    pub async fn handle_batch(&self, mut msgs: Vec<Message>) -> Vec<Result<(), ()>> {
        let resolved = self.resolver.resolve_all(&msgs).await;
        let Grouped {
            mut results,
            groups,
            poison,
        } = group_messages(&mut msgs, &resolved);
        for (idx, reason) in poison {
            if let Err(e) = self.route_dlq(&msgs[idx], &reason).await {
                tracing::error!(error = %e,message_index = idx,"failed to route poison message to DLQ");
                results[idx] = Err(());
            }
        }

        for (setting_code, group) in groups {
            let outcome = match self.process_group(&msgs, &setting_code, &group).await {
                Ok(()) => Ok(()),
                Err(_) => Err(()),
            };

            for &idx in &group.idxs {
                results[idx] = outcome;
            }
        }

        results
    }

    async fn process_group(
        &self,
        msgs: &[Message],
        setting_code: &str,
        group: &Group,
    ) -> Result<(), ProcessError> {
        self.emit_group(msgs, setting_code, group).await?;
        self.emit_group_plugin(msgs, setting_code, group).await?;

        let payload = sonic_rs::to_vec(&group.batch).map_err(|e| {
            tracing::error!(error = %e,setting_code,"failed to serialize group result");
            ProcessError::Payload(e.to_string())
        })?;

        self.producer
            .send_objects(setting_code, group.out_key.as_deref(), payload)
            .await
            .map_err(|e| {
                tracing::error!(error = %e,setting_code,"failed to publish group result");
                ProcessError::Producer(e.to_string())
            })?;

        Ok(())
    }

    async fn emit_group(
        &self,
        msgs: &[Message],
        setting_code: &str,
        group: &Group,
    ) -> Result<(), ProcessError> {
        match chain::execute_batch(&self.runtime, &group.batch, &group.settings).await {
            Ok(_) => Ok(()),
            Err(e) if e.is_transient() => {
                tracing::warn!(error = %e,setting_code,"group transient failure");
                Err(e)
            }

            Err(e) => {
                let reason = e.to_string();
                tracing::warn!(error = %reason,setting_code,"group poison; routing to DLQ");
                self.route_group_dlq(msgs, group, &reason).await
            }
        }
    }

    async fn emit_group_plugin(
        &self,
        msgs: &[Message],
        setting_code: &str,
        group: &Group,
    ) -> Result<(), ProcessError> {
        match chain::execute_plugin(&self.plugin, &group.batch, &group.settings).await {
            Ok(_) => Ok(()),

            Err(e) if e.is_transient() => {
                tracing::warn!(error = %e,setting_code,"group plugin transient failure");
                Err(e)
            }

            Err(e) => {
                let reason = e.to_string();
                tracing::warn!(error = %reason,setting_code,"group plugin poison; routing to DLQ");
                self.route_group_dlq(msgs, group, &reason).await
            }
        }
    }

    async fn route_group_dlq(
        &self,
        msgs: &[Message],
        group: &Group,
        reason: &str,
    ) -> Result<(), ProcessError> {
        futures::future::try_join_all(
            group
                .idxs
                .iter()
                .map(|&idx| self.route_dlq(&msgs[idx], reason)),
        )
        .await?;

        Ok(())
    }

    async fn route_dlq(&self, msg: &Message, reason: &str) -> Result<(), ProcessError> {
        let Some(raw) = msg.raw.as_deref() else {
            return Ok(());
        };

        let ctx = DlqContext {
            reason,
            source_topic: msg.topic(),
            source_partition: msg.partition(),
            source_offset: msg.offset(),
        };

        self.producer
            .send_dlq(raw, msg.key.as_deref(), ctx)
            .await
            .map_err(|e| ProcessError::Producer(e.to_string()))
    }
}
