use crate::ark_host::{ArkHost, ArkResult};
use serde_json::Value;

/// Core#53 boundary: integration replication stays behind the existing ArkHost RPC.
/// Payloads stay opaque at this boundary; Core's RPC dispatcher owns schemas.
pub(crate) struct ReplicationConsumer<'a> {
    ark: &'a ArkHost,
}

impl<'a> ReplicationConsumer<'a> {
    pub(crate) fn new(ark: &'a ArkHost) -> Self {
        Self { ark }
    }

    pub(crate) async fn persist_node_authorization(
        &self,
        operation: &str,
        node: &Value,
        grant: Option<&Value>,
        device_id: &str,
    ) -> ArkResult<bool> {
        let response = self
            .ark
            .request(
                "integration.persist_node_authorization",
                serde_json::json!({
                    "authorization_operation": operation,
                    "node": node,
                    "grant": grant,
                    "device_id": device_id,
                }),
            )
            .await?;
        if !response.ok {
            return Err(crate::ark_host::ArkHostError::RpcError(
                response
                    .error
                    .unwrap_or_else(|| "Core node authorization failed".into()),
            ));
        }
        serde_json::from_value(response.data).map_err(crate::ark_host::ArkHostError::Json)
    }

    pub(crate) async fn persist_integration_grant(
        &self,
        grant: &Value,
        device_id: &str,
    ) -> ArkResult<bool> {
        let response = self
            .ark
            .request(
                "integration.persist_integration_grant",
                serde_json::json!({
                    "grant": grant,
                    "device_id": device_id,
                }),
            )
            .await?;
        if !response.ok {
            return Err(crate::ark_host::ArkHostError::RpcError(
                response
                    .error
                    .unwrap_or_else(|| "Core integration grant failed".into()),
            ));
        }
        serde_json::from_value(response.data).map_err(crate::ark_host::ArkHostError::Json)
    }

    pub(crate) async fn prepare_signed_sync(
        &self,
        space_id: &str,
        origin_node_id: &str,
        integration_id: &str,
        recipient_node_id: &str,
        message_id: &str,
    ) -> ArkResult<Value> {
        let response = self
            .ark
            .request(
                "integration.prepare_signed_sync",
                serde_json::json!({
                    "space_id": space_id,
                    "origin_node_id": origin_node_id,
                    "integration_id": integration_id,
                    "recipient_node_id": recipient_node_id,
                    "message_id": message_id,
                }),
            )
            .await?;
        if !response.ok {
            return Err(crate::ark_host::ArkHostError::RpcError(
                response
                    .error
                    .unwrap_or_else(|| "Core sync preparation failed".into()),
            ));
        }
        Ok(response.data)
    }

    pub(crate) async fn validate_outbound_signed_sync(
        &self,
        space_id: &str,
        origin_node_id: &str,
        frame: &Value,
    ) -> ArkResult<bool> {
        let response = self
            .ark
            .request(
                "integration.validate_outbound_signed_sync",
                serde_json::json!({
                    "space_id": space_id,
                    "origin_node_id": origin_node_id,
                    "frame": frame,
                }),
            )
            .await?;
        if !response.ok {
            return Err(crate::ark_host::ArkHostError::RpcError(
                response
                    .error
                    .unwrap_or_else(|| "Core outbound sync validation failed".into()),
            ));
        }
        serde_json::from_value(response.data).map_err(crate::ark_host::ArkHostError::Json)
    }

    pub(crate) async fn send_signed_sync(&self, frame: &Value) -> ArkResult<bool> {
        let response = self
            .ark
            .request(
                "integration.send_signed_sync",
                serde_json::json!({ "frame": frame }),
            )
            .await?;
        if !response.ok {
            return Err(crate::ark_host::ArkHostError::RpcError(
                response
                    .error
                    .unwrap_or_else(|| "Core addressed sync send failed".into()),
            ));
        }
        serde_json::from_value(response.data).map_err(crate::ark_host::ArkHostError::Json)
    }

    pub(crate) async fn acquire_refresh_lease(&self, params: &Value) -> ArkResult<Value> {
        self.request("integration.acquire_refresh_lease", params)
            .await
    }

    pub(crate) async fn publish_credential_envelope(&self, params: &Value) -> ArkResult<bool> {
        let response = self
            .ark
            .request("integration.publish_credential_envelope", params.clone())
            .await?;
        if !response.ok {
            return Err(crate::ark_host::ArkHostError::RpcError(
                response
                    .error
                    .unwrap_or_else(|| "Core credential publication failed".into()),
            ));
        }
        serde_json::from_value(response.data).map_err(crate::ark_host::ArkHostError::Json)
    }

    pub(crate) async fn load_latest_credential_envelope(&self, params: &Value) -> ArkResult<Value> {
        self.request("integration.load_latest_credential_envelope", params)
            .await
    }

    pub(crate) async fn lookup_issuer_encryption_key(&self, params: &Value) -> ArkResult<Value> {
        self.request("integration.lookup_issuer_encryption_key", params)
            .await
    }

    pub(crate) async fn check_credential_fence(&self, params: &Value) -> ArkResult<bool> {
        self.request("integration.check_credential_fence", params)
            .await
            .and_then(|value| {
                serde_json::from_value(value).map_err(crate::ark_host::ArkHostError::Json)
            })
    }

    pub(crate) async fn lookup_issuer_encryption_key_for_publish(
        &self,
        params: &Value,
    ) -> ArkResult<Value> {
        self.request(
            "integration.lookup_issuer_encryption_key_for_publish",
            params,
        )
        .await
    }

    pub(crate) async fn verification_status(&self, params: &Value) -> ArkResult<Value> {
        self.request("integration.verification_status", params)
            .await
    }

    async fn request(&self, operation: &str, params: &Value) -> ArkResult<Value> {
        let response = self.ark.request(operation, params.clone()).await?;
        if !response.ok {
            return Err(crate::ark_host::ArkHostError::RpcError(
                response
                    .error
                    .unwrap_or_else(|| "Core integration operation failed".into()),
            ));
        }
        Ok(response.data)
    }
}
