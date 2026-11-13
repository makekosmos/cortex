use super::{Connection, SqliteWriter, TransactionFailure};
use crate::sync_progress_write::{
    apply_sync_progress_write, SyncProgressWrite, SyncProgressWriteError,
};
use crate::writer::StoreTransactionFailure;

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

#[cfg(test)]
mod tests {
    use super::*;
    use domain::types::VersionVector;
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
