use ark_core::replication_storage::AuthenticatedEntityBackend as ModuleAuthenticatedEntityBackend;
use ark_core::AuthenticatedEntityBackend as RootAuthenticatedEntityBackend;

struct Backend;

#[async_trait::async_trait]
impl RootAuthenticatedEntityBackend for Backend {
    async fn receive_authenticated_entity_with_id(
        &self,
        _: &ark_core::types::SyncEntity,
        _: &str,
        _: &str,
    ) -> Result<bool, String> {
        Ok(false)
    }

    async fn receive_authenticated_entity_with_policy(
        &self,
        _: &ark_core::types::SyncEntity,
        _: &str,
        _: &str,
        _: &str,
        _: Option<ark_domain::sync_policy::PolicyIdentity>,
    ) -> Result<bool, String> {
        Ok(false)
    }

    async fn apply_authenticated_entity(
        &self,
        _: &ark_core::types::SyncEntity,
        _: &str,
    ) -> Result<(), String> {
        Err("unused compatibility test backend".into())
    }

    async fn apply_authenticated_entity_with_id(
        &self,
        _: &ark_core::types::SyncEntity,
        _: &str,
        _: &str,
    ) -> Result<ark_application::sync_storage::InboundApplyOutcome, String> {
        Err("unused compatibility test backend".into())
    }

    async fn apply_authenticated_entity_with_policy(
        &self,
        _: &ark_core::types::SyncEntity,
        _: &str,
        _: &str,
        _: &str,
        _: Option<ark_domain::sync_policy::PolicyIdentity>,
    ) -> Result<ark_application::sync_storage::InboundApplyOutcome, String> {
        Err("unused compatibility test backend".into())
    }
}

fn accepts_previous_module_path<T: ModuleAuthenticatedEntityBackend>() {}

#[test]
fn authenticated_storage_port_keeps_both_public_paths() {
    accepts_previous_module_path::<Backend>();
}

struct PeerBackend;

#[async_trait::async_trait]
impl ark_core::PeerIdentityBackend for PeerBackend {
    async fn load_local_sync_profile(
        &self,
        _local_device_id: &str,
    ) -> Result<Option<ark_application::sync_policy::SyncProfile>, String> {
        Ok(None)
    }
    async fn accept_peer_sync_policy(
        &self,
        _peer_device_id: &str,
        _revision: i64,
        _digest: &str,
        _active: bool,
        _rules: &[ark_application::sync_policy::SyncProfileRule],
    ) -> Result<ark_application::sync_policy::AcceptedPeerSyncPolicy, String> {
        Err("unused compatibility test backend".into())
    }
    async fn load_accepted_peer_sync_policy(
        &self,
        _peer_device_id: &str,
    ) -> Result<Option<ark_application::sync_policy::AcceptedPeerSyncPolicy>, String> {
        Ok(None)
    }
    async fn sync_session_is_admitted(&self, _peer_device_id: &str) -> Result<bool, String> {
        Ok(false)
    }
    async fn registry_device_is_enabled(
        &self,
        _peer_device_id: &str,
    ) -> Result<Option<bool>, String> {
        Ok(None)
    }
    async fn enroll_paired_device(
        &self,
        _local_device_id: &str,
        _target_device_id: &str,
        _transport_public_key: &str,
    ) -> Result<(), String> {
        Err("unused compatibility test backend".into())
    }
    async fn load_device_registry_events_page(
        &self,
        _after_position: i64,
        _high_watermark: Option<i64>,
        _limit: usize,
    ) -> Result<(i64, usize, Vec<ark_domain::device_registry::RegistryEvent>), String> {
        Err("unused compatibility test backend".into())
    }
    async fn apply_device_registry_events(
        &self,
        _sender_device_id: &str,
        _events: &[ark_domain::device_registry::RegistryEvent],
    ) -> Result<ark_core::replication_storage::RegistryPageApply, String> {
        Err("unused compatibility test backend".into())
    }
    async fn sign_registry_handshake(
        &self,
        _local_device_id: &str,
        _transcript: &ark_domain::device_registry::RegistryHandshakeTranscript,
    ) -> Result<ark_domain::device_registry::RegistryHandshakeProof, String> {
        Err("unused compatibility test backend".into())
    }
    async fn verify_registry_handshake(
        &self,
        _peer_device_id: &str,
        _transcript: &ark_domain::device_registry::RegistryHandshakeTranscript,
        _proof: &ark_domain::device_registry::RegistryHandshakeProof,
    ) -> Result<(), String> {
        Err("unused compatibility test backend".into())
    }

    async fn authorized_transport_public_key(
        &self,
        _device_id: &str,
    ) -> Result<Option<String>, String> {
        Ok(None)
    }

    async fn load_peer_transport_keys(
        &self,
    ) -> Result<std::collections::HashMap<String, String>, String> {
        Ok(std::collections::HashMap::new())
    }

    async fn store_peer_transport_keys(
        &self,
        _keys: &std::collections::HashMap<String, String>,
    ) -> Result<(), String> {
        Ok(())
    }
}

fn accepts_previous_peer_identity_path<T: ark_core::replication_storage::PeerIdentityBackend>() {}

#[test]
fn peer_identity_port_keeps_both_public_paths() {
    accepts_previous_peer_identity_path::<PeerBackend>();
}

#[test]
fn application_read_model_and_domain_type_registry_keep_ark_core_paths() {
    let _: fn(ark_application::load_all_data::LoadAllData) -> ark_core::types::LoadAllData =
        |value| value;
    let _: fn(ark_domain::canonical_type::TypeSummary) -> ark_core::type_registry::TypeSummary =
        |value| value;
    let _: fn(ark_domain::canonical_type::TypeVersion) -> ark_core::type_registry::TypeVersion =
        |value| value;
    let _: fn(
        ark_application::type_registry::TypeGetResponse,
    ) -> ark_core::type_registry::TypeGetResponse = |value| value;
}
