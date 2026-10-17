use ark_core::replication_storage::AuthenticatedEntityBackend as ModuleAuthenticatedEntityBackend;
use ark_core::AuthenticatedEntityBackend as RootAuthenticatedEntityBackend;

struct Backend;

impl RootAuthenticatedEntityBackend for Backend {}

fn accepts_previous_module_path<T: ModuleAuthenticatedEntityBackend>() {}

#[test]
fn authenticated_storage_port_keeps_both_public_paths() {
    accepts_previous_module_path::<Backend>();
}

struct PeerBackend;

#[async_trait::async_trait]
impl ark_core::PeerIdentityBackend for PeerBackend {
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
}
