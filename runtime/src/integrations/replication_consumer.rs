use crate::ark_host::{ArkHost, ArkResult};
use serde_json::Value;

/// Core#53 boundary: integration replication stays behind the existing ArkHost RPC.
/// Payloads are JSON strings because Core's FFI contract owns their schemas.
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
                "persist_node_authorization_json",
                serde_json::json!({
                    "authorizationOperation": operation,
                    "nodeJson": serde_json::to_string(node)?,
                    "grantJson": grant.map(serde_json::to_string).transpose()?,
                    "deviceId": device_id,
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
                "persist_integration_grant_json",
                serde_json::json!({
                    "grantJson": serde_json::to_string(grant)?,
                    "deviceId": device_id,
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
                "prepare_signed_sync_json",
                serde_json::json!({
                    "spaceId": space_id,
                    "originNodeId": origin_node_id,
                    "integrationId": integration_id,
                    "recipientNodeId": recipient_node_id,
                    "messageId": message_id,
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
        let json = response.data.as_str().ok_or_else(|| {
            crate::ark_host::ArkHostError::RpcError("Core sync preparation was not JSON".into())
        })?;
        serde_json::from_str(json).map_err(crate::ark_host::ArkHostError::Json)
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
                "validate_outbound_signed_sync_json",
                serde_json::json!({
                    "spaceId": space_id,
                    "originNodeId": origin_node_id,
                    "frameJson": serde_json::to_string(frame)?,
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
