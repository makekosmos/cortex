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
