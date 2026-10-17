use ark_core::replication_storage::AuthenticatedEntityBackend as ModuleAuthenticatedEntityBackend;
use ark_core::AuthenticatedEntityBackend as RootAuthenticatedEntityBackend;

struct Backend;

impl RootAuthenticatedEntityBackend for Backend {}

fn accepts_previous_module_path<T: ModuleAuthenticatedEntityBackend>() {}

#[test]
fn authenticated_storage_port_keeps_both_public_paths() {
    accepts_previous_module_path::<Backend>();
}
