use ark_domain::types::SyncEntity;

/// Applies an ordinary entity only after the sync boundary authenticated its peer.
#[async_trait::async_trait]
pub trait AuthenticatedEntityBackend: Send + Sync {
    /// Commits the bounded inbox receipt before applying the payload. Returns
    /// `true` only when this backend provides that durable boundary.
    async fn receive_authenticated_entity_with_id(
        &self,
        _entity: &SyncEntity,
        _authenticated_peer_id: &str,
        _message_id: &str,
    ) -> Result<bool, String> {
        Ok(false)
    }

    async fn receive_authenticated_entity_with_policy(
        &self,
        entity: &SyncEntity,
        authenticated_peer_id: &str,
        message_id: &str,
        _local_device_id: &str,
        policy_identity: Option<crate::sync_storage::PolicyIdentity>,
    ) -> Result<bool, String> {
        if policy_identity.is_some() {
            return Err("policy_bound_receipt_unsupported".into());
        }
        self.receive_authenticated_entity_with_id(entity, authenticated_peer_id, message_id)
            .await
    }

    async fn apply_authenticated_entity(
        &self,
        _entity: &SyncEntity,
        _authenticated_peer_id: &str,
    ) -> Result<(), String> {
        Err("authenticated_apply_unsupported".into())
    }

    async fn apply_authenticated_entity_with_id(
        &self,
        entity: &SyncEntity,
        authenticated_peer_id: &str,
        _message_id: &str,
    ) -> Result<crate::sync_storage::InboundApplyOutcome, String> {
        self.apply_authenticated_entity(entity, authenticated_peer_id)
            .await
            .map(|()| crate::sync_storage::InboundApplyOutcome::Applied)
    }

    async fn apply_authenticated_entity_with_policy(
        &self,
        entity: &SyncEntity,
        authenticated_peer_id: &str,
        message_id: &str,
        _local_device_id: &str,
        policy_identity: Option<crate::sync_storage::PolicyIdentity>,
    ) -> Result<crate::sync_storage::InboundApplyOutcome, String> {
        if policy_identity.is_some() {
            return Err("policy_bound_apply_unsupported".into());
        }
        self.apply_authenticated_entity_with_id(entity, authenticated_peer_id, message_id)
            .await
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryPageApply {
    pub digest: String,
    pub sender_is_admitted: bool,
    pub local_device_is_admitted: bool,
    pub disabled_device_ids: Vec<String>,
    pub applied_events: Vec<ark_domain::device_registry::RegistryEvent>,
}

/// Trusted transport identity records used while authenticating a paired peer.
#[async_trait::async_trait]
pub trait PeerIdentityBackend: Send + Sync {
    async fn load_local_sync_profile(
        &self,
        _local_device_id: &str,
    ) -> Result<Option<crate::sync_policy::SyncProfile>, String> {
        Ok(None)
    }

    async fn accept_peer_sync_policy(
        &self,
        _peer_device_id: &str,
        _revision: i64,
        _digest: &str,
        _active: bool,
        _rules: &[crate::sync_policy::SyncProfileRule],
    ) -> Result<crate::sync_policy::AcceptedPeerSyncPolicy, String> {
        Err("peer sync policy persistence is unsupported by this storage backend".into())
    }

    async fn load_accepted_peer_sync_policy(
        &self,
        _peer_device_id: &str,
    ) -> Result<Option<crate::sync_policy::AcceptedPeerSyncPolicy>, String> {
        Ok(None)
    }

    async fn sync_session_is_admitted(&self, _peer_device_id: &str) -> Result<bool, String> {
        Err("sync session admission lookup is unsupported by this storage backend".into())
    }

    async fn registry_device_is_enabled(
        &self,
        _peer_device_id: &str,
    ) -> Result<Option<bool>, String> {
        Err("device registry lookup is unsupported by this storage backend".into())
    }

    async fn authorized_transport_public_key(
        &self,
        device_id: &str,
    ) -> Result<Option<String>, String>;
    async fn load_peer_transport_keys(
        &self,
    ) -> Result<std::collections::HashMap<String, String>, String>;
    async fn store_peer_transport_keys(
        &self,
        keys: &std::collections::HashMap<String, String>,
    ) -> Result<(), String>;
    async fn enroll_paired_device(
        &self,
        local_device_id: &str,
        target_device_id: &str,
        transport_public_key: &str,
    ) -> Result<(), String> {
        let _ = (local_device_id, target_device_id, transport_public_key);
        Err("signed device registry enrollment is unsupported by this storage backend".into())
    }

    async fn load_device_registry_events_page(
        &self,
        after_position: i64,
        high_watermark: Option<i64>,
        limit: usize,
    ) -> Result<(i64, usize, Vec<ark_domain::device_registry::RegistryEvent>), String> {
        let _ = (after_position, high_watermark, limit);
        Err("signed device registry history is unsupported by this storage backend".into())
    }

    async fn apply_device_registry_events(
        &self,
        sender_device_id: &str,
        events: &[ark_domain::device_registry::RegistryEvent],
    ) -> Result<RegistryPageApply, String> {
        let _ = (sender_device_id, events);
        Err("signed device registry history is unsupported by this storage backend".into())
    }

    async fn sign_registry_handshake(
        &self,
        _local_device_id: &str,
        _transcript: &ark_domain::device_registry::RegistryHandshakeTranscript,
    ) -> Result<ark_domain::device_registry::RegistryHandshakeProof, String> {
        Err("registry handshake signing is unsupported by this storage backend".into())
    }

    async fn verify_registry_handshake(
        &self,
        _peer_device_id: &str,
        _transcript: &ark_domain::device_registry::RegistryHandshakeTranscript,
        _proof: &ark_domain::device_registry::RegistryHandshakeProof,
    ) -> Result<(), String> {
        Err("registry handshake verification is unsupported by this storage backend".into())
    }
}
