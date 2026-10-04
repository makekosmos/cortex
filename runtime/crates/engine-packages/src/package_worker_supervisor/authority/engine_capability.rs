use super::authority::ArkRequestExecutor;
use crate::dictation::{handle_dictation_op, DictationHost};
use async_trait::async_trait;
use std::sync::Arc;

/// Host-side autostart toggle used by `dictation.lifecycle.set_autostart`.
/// Implemented by `ManagerState` in the engine crate (KOS-336: the executor
/// lives here while ManagerState stays behind the manager_api boundary).
pub trait AutostartControl: Send + Sync {
    fn set_autostart(&self, enabled: bool) -> Result<serde_json::Value, String>;
}

/// Routes worker calls through the same Engine-owned capabilities used by
/// Host clients. Dictation workers never receive a direct microphone or
/// window handle; they only get the scoped operation result.
pub struct EngineCapabilityExecutor {
    ark: Arc<dyn ArkRequestExecutor>,
    dictation: Arc<DictationHost>,
    manager: Arc<dyn AutostartControl>,
}

impl EngineCapabilityExecutor {
    pub fn new(
        ark: Arc<dyn ArkRequestExecutor>,
        dictation: Arc<DictationHost>,
        manager: Arc<dyn AutostartControl>,
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

    struct NoopAutostart;

    impl AutostartControl for NoopAutostart {
        fn set_autostart(&self, enabled: bool) -> Result<serde_json::Value, String> {
            Ok(json!({"enabled": enabled}))
        }
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
            Arc::new(NoopAutostart),
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
