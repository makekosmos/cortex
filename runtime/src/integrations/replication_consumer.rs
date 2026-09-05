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
}
