use super::authority::ArkRequestExecutor;
use crate::dictation::{handle_dictation_op, DictationHost};
use crate::manager_api::ManagerState;
use async_trait::async_trait;
use std::sync::Arc;

/// Routes worker calls through the same Engine-owned capabilities used by
/// Host clients. Dictation workers never receive a direct microphone or
/// window handle; they only get the scoped operation result.
pub struct EngineCapabilityExecutor {
    ark: Arc<dyn ArkRequestExecutor>,
    dictation: Arc<DictationHost>,
    manager: ManagerState,
}

impl EngineCapabilityExecutor {
    pub fn new(
        ark: Arc<dyn ArkRequestExecutor>,
        dictation: Arc<DictationHost>,
        manager: ManagerState,
    ) -> Self {
        Self {
            ark,
            dictation,
            manager,
        }
    }
}

#[async_trait]
impl ArkRequestExecutor for EngineCapabilityExecutor {
    async fn request(
        &self,
        operation: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, &'static str> {
        if let Some(subop) = operation.strip_prefix("dictation.") {
            if subop == "lifecycle.set_autostart" {
                return self
                    .manager
                    .set_autostart(
                        params
                            .get("enabled")
                            .and_then(serde_json::Value::as_bool)
                            .unwrap_or(false),
                    )
                    .map_err(|_| "unavailable");
            }
            let response = handle_dictation_op(subop, params, &self.dictation).await;
            return response.ok.then_some(response.data).ok_or("unavailable");
        }
        self.ark.request(operation, params).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictation::{DictationConfig, DictationHost};
    use serde_json::json;
    use std::sync::Mutex;

    struct RecordingArk {
        operations: Mutex<Vec<String>>,
    }

    #[async_trait]
    impl ArkRequestExecutor for RecordingArk {
        async fn request(
            &self,
            operation: &str,
            _params: serde_json::Value,
        ) -> Result<serde_json::Value, &'static str> {
            self.operations.lock().unwrap().push(operation.to_owned());
            Ok(json!({"delegated": true}))
        }
    }

    #[tokio::test]
    async fn routes_dictation_to_engine_host_and_other_operations_to_ark() {
        let data_dir = tempfile::tempdir().unwrap();
        let fallback = Arc::new(RecordingArk {
            operations: Mutex::new(Vec::new()),
        });
        let executor = EngineCapabilityExecutor::new(
            fallback.clone(),
            DictationHost::new_for_test(
                data_dir.path().to_path_buf(),
                "http://localhost/".into(),
                DictationConfig::default(),
            ),
            ManagerState::new(data_dir.path().to_path_buf()),
        );

        let state = executor
            .request("dictation.get_state", json!({}))
            .await
            .unwrap();
        assert_eq!(state["state"], "idle");
        let delegated = executor.request("ark.custom", json!({})).await.unwrap();
        assert_eq!(delegated["delegated"], true);
        assert_eq!(
            fallback.operations.lock().unwrap().as_slice(),
            ["ark.custom"]
        );
    }
}
