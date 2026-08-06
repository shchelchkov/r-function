
#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use r_error::runtime::error::RuntimeError;
    use r_producer::kafka::producer::{DlqContext, KafkaSendError};
    use r_setting::functions::function_setting::FunctionSetting;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use r_process::process::{Message, MessagePublisher, Processor, SettingProvider};
    use r_runtime_api::Runtime;

    #[derive(Default)]
    struct MockPublisher {
        sent: Mutex<Vec<(String, Option<Vec<u8>>)>>,
        dlq: Mutex<Vec<String>>,
    }

    #[async_trait]
    impl MessagePublisher for MockPublisher {
        async fn send_objects(
            &self,
            setting_code: &str,
            key: Option<&[u8]>,
            _payload: Vec<u8>,
        ) -> Result<(), KafkaSendError> {
            self.sent
                .lock()
                .unwrap()
                .push((setting_code.to_string(), key.map(<[u8]>::to_vec)));
            Ok(())
        }

        async fn send_dlq(
            &self,
            _payload: &[u8],
            _key: Option<&[u8]>,
            ctx: DlqContext<'_>,
        ) -> Result<(), KafkaSendError> {
            self.dlq.lock().unwrap().push(ctx.reason.to_string());
            Ok(())
        }
    }

    struct MockProvider {
        settings: Arc<Vec<FunctionSetting>>,
        value_key: Arc<Vec<String>>,
    }

    impl SettingProvider for MockProvider {
        fn get_cached_setting(&self, _sc: &str) -> Option<Arc<Vec<FunctionSetting>>> {
            Some(self.settings.clone())
        }
        fn get_function_setting(&self, _sc: &str) -> Option<Arc<Vec<FunctionSetting>>> {
            Some(self.settings.clone())
        }
        fn get_value_key(&self, _sc: &str) -> Option<Arc<Vec<String>>> {
            Some(self.value_key.clone())
        }
    }

    struct MockRuntime {
        out: Vec<u8>,
    }

    #[async_trait]
    impl Runtime for MockRuntime {
        async fn invoke_raw(
            &self,
            _module: &str,
            _payload: Vec<u8>,
        ) -> Result<Vec<u8>, RuntimeError> {
            Ok(self.out.clone())
        }
    }

    fn fs(json: &str) -> FunctionSetting {
        sonic_rs::from_slice(json.as_bytes()).expect("valid function setting")
    }

    fn block_on<F: std::future::Future>(fut: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(fut)
    }

    #[test]
    fn handle_batch_publishes_group_result() {
        let publisher = Arc::new(MockPublisher::default());
        let provider = Arc::new(MockProvider {
            settings: Arc::new(vec![fs(
                r#"{"isActive":true,"isKey":true,"key":"topic","module":["m1"]}"#,
            )]),
            value_key: Arc::new(vec!["topic".to_string()]),
        });
        let runtime: Arc<dyn Runtime> = Arc::new(MockRuntime {
            out: br#"[{"r":1}]"#.to_vec(),
        });

        let processor = Processor::new(publisher.clone(), provider, runtime);

        let msg = Message::new(
            "src".into(),
            0,
            7,
            Some(b"t1".to_vec()),
            Some(br#"{"setting_code":"sc","topic":"t1"}"#.to_vec()),
            HashMap::new(),
            None,
        );

        let results = block_on(processor.handle_batch(vec![msg]));

        assert_eq!(results, vec![Ok(())], "успешная группа -> коммит офсета");
        let sent = publisher.sent.lock().unwrap();
        assert_eq!(sent.len(), 1, "одна группа -> одна публикация");
        assert_eq!(sent[0].0, "sc", "опубликовано под своим setting_code");
        assert_eq!(
            sent[0].1.as_deref(),
            Some(&b"t1"[..]),
            "out_key из сообщения"
        );
        assert!(
            publisher.dlq.lock().unwrap().is_empty(),
            "без poison :: без DLQ"
        );
    }

    #[test]
    fn handle_batch_routes_poison_to_dlq() {
        let publisher = Arc::new(MockPublisher::default());
        let provider = Arc::new(MockProvider {
            settings: Arc::new(vec![fs(
                r#"{"isActive":true,"isKey":true,"key":"topic","module":["m1"]}"#,
            )]),
            value_key: Arc::new(vec!["topic".to_string()]),
        });
        let runtime: Arc<dyn Runtime> = Arc::new(MockRuntime { out: vec![] });
        let processor = Processor::new(publisher.clone(), provider, runtime);

        let msg = Message::new(
            "src".into(),
            0,
            7,
            None,
            Some(br#"{"setting_code":"sc","topic":"t1",}"#.to_vec()),
            HashMap::new(),
            None,
        );

        let results = block_on(processor.handle_batch(vec![msg]));

        assert_eq!(results, vec![Ok(())], "poison отправлено в DLQ -> коммит");
        assert_eq!(
            publisher.dlq.lock().unwrap().len(),
            1,
            "одно poison один DLQ"
        );
        assert!(
            publisher.sent.lock().unwrap().is_empty(),
            "poison не публикуется"
        );
    }
}
