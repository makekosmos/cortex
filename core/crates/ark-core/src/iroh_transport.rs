#[cfg(test)]
use rusqlite::Connection;

#[cfg(test)]
const IROH_SECRET_KEY_KV_KEY: &str = "iroh.secret_key";

#[cfg(test)]
struct SqliteIrohIdentityStore<'a>(&'a Connection);

#[cfg(test)]
impl ark_net::iroh::IrohIdentityStore for SqliteIrohIdentityStore<'_> {
    fn load_secret_key(&self) -> Result<Option<String>, String> {
        ark_store::sync_kv::get_sync_kv(self.0, IROH_SECRET_KEY_KV_KEY)
    }

    fn insert_secret_key_if_absent(&self, candidate_hex: &str) -> Result<String, String> {
        ark_store::sync_kv::get_or_insert_sync_kv(self.0, IROH_SECRET_KEY_KV_KEY, candidate_hex)
    }
}

#[cfg(test)]
pub fn load_or_generate_secret_key(
    connection: &Connection,
) -> Result<ark_net::iroh::SecretKey, String> {
    ark_net::iroh::load_or_create_secret_key(&SqliteIrohIdentityStore(connection))
}

pub(crate) async fn load_or_generate_secret_key_via_actor(
    db: &crate::db::DbHandle,
) -> Result<ark_net::iroh::SecretKey, String> {
    let candidate = ark_net::iroh::generate_secret_key_candidate();
    let persisted = db
        .submit_iroh_identity_key_candidate(crate::db::CommandContext::engine(), candidate)
        .await
        .map_err(|error| error.to_string())?;
    ark_net::iroh::parse_secret_key_hex(&persisted)
}

pub use ark_net::iroh::*;

#[cfg(test)]
#[path = "iroh_transport/bidirectional_tests.rs"]
mod bidirectional_tests;
#[cfg(test)]
#[path = "iroh_transport/stop_teardown_tests.rs"]
mod stop_teardown_tests;
