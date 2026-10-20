use rusqlite::Connection;

const IROH_SECRET_KEY_KV_KEY: &str = "iroh.secret_key";

struct SqliteIrohIdentityStore<'a>(&'a Connection);

impl ark_net::iroh::IrohIdentityStore for SqliteIrohIdentityStore<'_> {
    fn load_secret_key(&self) -> Result<Option<String>, String> {
        ark_store::sync_kv::get_sync_kv(self.0, IROH_SECRET_KEY_KV_KEY)
    }

    fn insert_secret_key_if_absent(&self, candidate_hex: &str) -> Result<String, String> {
        ark_store::sync_kv::get_or_insert_sync_kv(self.0, IROH_SECRET_KEY_KV_KEY, candidate_hex)
    }
}

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
mod tests {
    use rusqlite::Connection;

    use super::load_or_generate_secret_key;

    fn setup_db() -> Connection {
        let connection = Connection::open_in_memory().expect("database opens");
        ark_store::open::init_schema(&connection).expect("schema initializes");
        connection
    }

    #[test]
    fn secret_key_is_persisted_across_calls_on_same_storage() {
        let connection = setup_db();
        let first = load_or_generate_secret_key(&connection).expect("first load/generate");
        let second = load_or_generate_secret_key(&connection).expect("second load/generate");
        assert_eq!(first.public(), second.public());
    }

    #[test]
    fn secret_key_differs_across_independent_storages() {
        let first_connection = setup_db();
        let second_connection = setup_db();
        let first = load_or_generate_secret_key(&first_connection).expect("first identity");
        let second = load_or_generate_secret_key(&second_connection).expect("second identity");
        assert_ne!(first.public(), second.public());
    }

    #[test]
    fn first_call_persists_key_in_sync_kv() {
        let connection = setup_db();
        assert_eq!(
            ark_store::sync_kv::get_sync_kv(&connection, super::IROH_SECRET_KEY_KV_KEY)
                .expect("missing key reads"),
            None
        );
        load_or_generate_secret_key(&connection).expect("identity generates");
        assert!(
            ark_store::sync_kv::get_sync_kv(&connection, super::IROH_SECRET_KEY_KV_KEY)
                .expect("persisted key reads")
                .is_some()
        );
    }
}
