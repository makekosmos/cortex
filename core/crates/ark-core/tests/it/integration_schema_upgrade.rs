use ark_core::{db, schema::CREATE_TABLES};
use rusqlite::Connection;

#[test]
fn legacy_integration_columns_upgrade_idempotently_without_losing_envelopes() {
    let conn = Connection::open_in_memory().expect("fixture database");
    let legacy_schema = CREATE_TABLES
        .replace("    transport_public_key TEXT,\n", "")
        .replace("    refresh_fencing_token INTEGER NOT NULL,\n", "");
    conn.execute_batch(&legacy_schema).expect("legacy schema");
    conn.execute_batch(
        "INSERT INTO authorized_nodes VALUES
         ('node', 'fingerprint', 'signing', 'encryption', 1, 'active', 'date', NULL, NULL, 1, \
         'hlc');
         INSERT INTO integration_credential_envelopes VALUES
         ('envelope', 'integration', 'node', 1, 1, 'key', 'algorithm', 'nonce',
          'opaque-fixture', NULL, 'issuer', 'date', 1, 'hlc');",
    )
    .expect("legacy rows");
    for _ in 0..2 {
        db::init_schema(&conn).expect("repeatable upgrade");
    }
    let (ciphertext, fence): (String, i64) = conn
        .query_row(
            "SELECT ciphertext, refresh_fencing_token FROM integration_credential_envelopes
             WHERE envelope_id = 'envelope'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("preserved envelope");
    assert_eq!(ciphertext, "opaque-fixture");
    assert_eq!(fence, 0, "legacy envelopes must not acquire a valid fence");
    let transport_key: Option<String> = conn
        .query_row(
            "SELECT transport_public_key FROM authorized_nodes WHERE node_id = 'node'",
            [],
            |row| row.get(0),
        )
        .expect("preserved node");
    assert_eq!(transport_key, None, "migration must not invent peer trust");
}
