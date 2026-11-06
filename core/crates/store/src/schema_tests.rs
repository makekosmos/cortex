use rusqlite::Connection;

use crate::schema::CREATE_TABLES;

#[test]
fn schema_creates_shared_storage_tables_and_can_reopen() {
    let directory = crate::test_support::test_temp_dir().expect("temporary directory opens");
    let path = directory.path().join("ark.db");

    {
        let connection = Connection::open(&path).expect("database opens");
        connection
            .execute_batch(CREATE_TABLES)
            .expect("schema applies");
        connection
            .execute(
                "INSERT INTO sync_kv(key, value) VALUES ('probe', 'kept')",
                [],
            )
            .expect("probe row writes");
    }

    let connection = Connection::open(&path).expect("database reopens");
    connection
        .execute_batch(CREATE_TABLES)
        .expect("schema reapplies");
    let value: String = connection
        .query_row("SELECT value FROM sync_kv WHERE key = 'probe'", [], |row| {
            row.get(0)
        })
        .expect("probe row survives reopen");
    assert_eq!(value, "kept");
    for table in [
        "objects",
        "ark_operation_receipts",
        "integration_node_grants",
        "device_registry_events",
    ] {
        let exists: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
                [table],
                |row| row.get(0),
            )
            .expect("table lookup succeeds");
        assert!(exists, "missing {table}");
    }
}
