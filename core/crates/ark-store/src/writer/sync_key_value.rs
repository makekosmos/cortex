use super::{Connection, SqliteWriter, Transaction, TransactionFailure};
use crate::writer::StoreTransactionFailure;
use ark_application::sync_storage::{SyncProgressWrite, SyncProgressWriteError};

impl SqliteWriter {
    pub fn set_local_sync_key_value(
        &mut self,
        key: &str,
        value: &str,
    ) -> Result<(), TransactionFailure<String>> {
        Self::immediate_on_connection(&mut self.connection, |transaction| {
            crate::sync_kv::set_sync_kv(transaction, key, value)
        })
    }

    pub fn set_local_sync_key_value_on_connection(
        connection: &mut Connection,
        key: &str,
        value: &str,
    ) -> Result<(), TransactionFailure<String>> {
        Self::immediate_on_connection(connection, |transaction| {
            crate::sync_kv::set_sync_kv(transaction, key, value)
        })
    }

    pub fn set_sync_progress(
        &mut self,
        write: SyncProgressWrite,
    ) -> Result<(), StoreTransactionFailure<SyncProgressWriteError>> {
        self.write_with_capabilities(|capabilities| {
            apply_sync_progress_write(capabilities.transaction, write)
        })
    }

    pub fn set_sync_progress_on_connection(
        connection: &mut Connection,
        write: SyncProgressWrite,
    ) -> Result<(), TransactionFailure<SyncProgressWriteError>> {
        Self::immediate_on_connection(connection, |transaction| {
            apply_sync_progress_write(transaction, write)
        })
    }
}

fn apply_sync_progress_write(
    transaction: &Transaction<'_>,
    write: SyncProgressWrite,
) -> Result<(), SyncProgressWriteError> {
    struct Session<'scope, 'connection>(&'scope Transaction<'connection>);

    impl ark_application::sync_storage::SyncProgressWriteSession for Session<'_, '_> {
        fn load_sync_profile_for_sync(
            &self,
            local_device_id: &str,
        ) -> Result<Option<ark_application::sync_policy::SyncProfile>, String> {
            crate::data_platform::load_sync_profile_for_sync(self.0, local_device_id)
        }

        fn set_sync_value(&self, key: &str, value: &str) -> Result<(), String> {
            crate::sync_kv::set_sync_kv(self.0, key, value)
        }

        fn merge_version_vector(
            &self,
            incoming: &ark_domain::types::VersionVector,
        ) -> Result<(), String> {
            let mut current = crate::sync_kv::get_sync_kv(self.0, crate::VERSION_VECTOR_KEY)?
                .map(|raw| serde_json::from_str(&raw).map_err(|error| error.to_string()))
                .transpose()?
                .unwrap_or_default();
            ark_application::sync_storage::merge_version_vector_entries(&mut current, incoming);
            let encoded = serde_json::to_string(&current).map_err(|error| error.to_string())?;
            crate::sync_kv::set_sync_kv(self.0, crate::VERSION_VECTOR_KEY, &encoded)
        }
    }

    let session = Session(transaction);
    ark_application::sync_storage::apply_sync_progress_write(&session, write)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ark_domain::types::VersionVector;
    use rusqlite::Connection;

    #[test]
    fn stale_progress_merge_preserves_newer_hlcs_and_usage_cursors() {
        let mut connection =
            Connection::open_in_memory().expect("in-memory SQLite connection should open");
        crate::open::init_schema(&connection).expect("fresh SQLite schema should initialize");

        let mut current = VersionVector::new();
        current.insert(
            "book-1".into(),
            "2026-10-16T00:00:02.000Z:000002:device-a".into(),
        );
        current.insert(
            "unrelated-entity".into(),
            "2026-10-16T00:00:03.000Z:000003:device-a".into(),
        );
        current.insert("@usage:device-a".into(), "10".into());
        crate::sync_kv::set_sync_kv(
            &connection,
            crate::VERSION_VECTOR_KEY,
            &serde_json::to_string(&current).expect("version vector should serialize"),
        )
        .expect("initial version vector should persist");

        let mut stale = VersionVector::new();
        stale.insert(
            "book-1".into(),
            "2026-10-16T00:00:01.000Z:000001:device-a".into(),
        );
        stale.insert(
            "unrelated-entity".into(),
            "2026-10-16T00:00:04.000Z:000004:device-a".into(),
        );
        stale.insert("@usage:device-a".into(), "9".into());
        stale.insert("@usage:device-b".into(), "10".into());

        SqliteWriter::set_sync_progress_on_connection(
            &mut connection,
            SyncProgressWrite::merge_version_vector(stale),
        )
        .expect("version vector merge should commit");

        let persisted: VersionVector = serde_json::from_str(
            &crate::sync_kv::get_sync_kv(&connection, crate::VERSION_VECTOR_KEY)
                .expect("stored version vector should be readable")
                .expect("merged version vector should exist"),
        )
        .expect("merged version vector should deserialize");
        assert_eq!(persisted.get("book-1"), current.get("book-1"));
        assert_eq!(
            persisted.get("unrelated-entity"),
            Some(&"2026-10-16T00:00:04.000Z:000004:device-a".to_owned())
        );
        assert_eq!(persisted.get("@usage:device-a"), Some(&"10".to_owned()));
        assert_eq!(persisted.get("@usage:device-b"), Some(&"10".to_owned()));
    }
}
