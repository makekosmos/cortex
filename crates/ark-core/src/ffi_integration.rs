use super::*;

#[uniffi::export]
impl ArkCore {
    pub fn persist_node_authorization_json(
        &self,
        authorization_operation: String,
        node_json: String,
        grant_json: Option<String>,
        device_id: String,
    ) -> Result<bool> {
        let node =
            serde_json::from_str::<crate::integration_replication::AuthorizedNode>(&node_json)
                .map_err(|_| malformed_json_error("/node"))?;
        let grant = grant_json
            .map(|value| serde_json::from_str(&value).map_err(|_| malformed_json_error("/grant")))
            .transpose()?;
        let operation = match authorization_operation.as_str() {
            "authorize" => crate::integration_replication::NodeAuthorizationOperation::Authorize,
            "revoke" => crate::integration_replication::NodeAuthorizationOperation::Revoke,
            "rotate" => crate::integration_replication::NodeAuthorizationOperation::Rotate,
            _ => {
                return Err(structured_error(
                    "invalid_request",
                    "invalid_operation",
                    Some("/authorizationOperation"),
                ))
            }
        };
        self.with_conn(|conn| {
            crate::integration_replication::persist_node_authorization(
                conn,
                operation,
                &node,
                grant.as_ref(),
                &device_id,
            )
            .map(|_| true)
            .map_err(ArkCoreError::from)
        })
    }

    pub fn persist_integration_grant_json(
        &self,
        grant_json: String,
        device_id: String,
    ) -> Result<bool> {
        let grant = serde_json::from_str::<crate::integration_replication::IntegrationNodeGrant>(
            &grant_json,
        )
        .map_err(|_| malformed_json_error("/grant"))?;
        self.with_conn(|conn| {
            crate::integration_replication::persist_integration_grant(conn, &grant, &device_id)
                .map(|_| true)
                .map_err(ArkCoreError::from)
        })
    }

    pub fn prepare_signed_sync_json(
        &self,
        space_id: String,
        origin_node_id: String,
        integration_id: String,
        recipient_node_id: String,
        message_id: String,
    ) -> Result<String> {
        self.with_conn(|conn| {
            let preparation = crate::integration_replication::prepare_signed_sync(
                conn,
                &space_id,
                &origin_node_id,
                &integration_id,
                &recipient_node_id,
                &message_id,
            )
            .map_err(ArkCoreError::from)?;
            serde_json::to_string(&preparation).map_err(|error| err(error.to_string()))
        })
    }

    pub fn validate_outbound_signed_sync_json(
        &self,
        space_id: String,
        origin_node_id: String,
        frame_json: String,
    ) -> Result<bool> {
        let frame =
            serde_json::from_str::<crate::integration_replication::SignedSyncEnvelope>(&frame_json)
                .map_err(|_| malformed_json_error("/frame"))?;
        self.with_conn(|conn| {
            crate::integration_replication::validate_outbound_signed_sync(
                conn,
                &space_id,
                &origin_node_id,
                &frame,
            )
            .map(|_| true)
            .map_err(ArkCoreError::from)
        })
    }

    /// Validate and route a host-signed frame only to its addressed
    /// authenticated peer. Core never accepts arbitrary inbound apply calls.
    pub fn send_signed_integration_frame_json(&self, frame_json: String) -> Result<bool> {
        let frame: crate::integration_replication::SignedSyncEnvelope =
            serde_json::from_str(&frame_json).map_err(|_| malformed_json_error("/frame"))?;
        self.runtime.block_on(async {
            let runtime = {
                let guard = self.sync.lock().await;
                guard
                    .as_ref()
                    .ok_or_else(|| err("sync not running"))?
                    .clone_refs()
            };
            self.with_conn(|conn| {
                crate::integration_replication::validate_outbound_signed_sync(
                    conn,
                    &runtime.space_id,
                    &runtime.device_id,
                    &frame,
                )
                .map_err(ArkCoreError::from)
            })?;
            if runtime
                .server
                .send_signed_integration_frame(&frame.recipient_node_id, frame.clone())
                .await
                .is_ok()
            {
                return Ok(true);
            }
            let clients: Vec<_> = runtime.clients.lock().await.values().cloned().collect();
            for client in clients {
                if client
                    .send_signed_integration_frame(frame.clone())
                    .await
                    .is_ok()
                {
                    return Ok(true);
                }
            }
            Err(err("target peer has no authenticated addressed route"))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ffi_rejects_malformed_node_json() {
        let core = ArkCore::new();
        let error = core
            .persist_node_authorization_json(
                "authorize".into(),
                "{}".into(),
                None,
                "ffi-test".into(),
            )
            .unwrap_err();
        let ArkCoreError::Generic(message) = error;
        assert!(message.contains("malformed_json"));
    }
}
