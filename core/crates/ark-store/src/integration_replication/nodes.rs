use ark_domain::integration_replication::{AuthorizedNode, NodeStatus};
use rusqlite::{params, Connection, OptionalExtension};

fn node_status_value(status: NodeStatus) -> &'static str {
    match status {
        NodeStatus::Active => "active",
        NodeStatus::Revoked => "revoked",
    }
}

pub fn upsert_authorized_node_record(
    connection: &Connection,
    node: &AuthorizedNode,
) -> Result<(), String> {
    let grant_epoch = sqlite_i64(node.grant_epoch, "grant_epoch")?;
    let revocation_epoch = node
        .revocation_epoch
        .map(|value| sqlite_i64(value, "revocation_epoch"))
        .transpose()?;
    let revision = sqlite_i64(node.revision, "revision")?;
    connection
        .execute(
            "INSERT INTO authorized_nodes
             (node_id, key_fingerprint, signing_public_key, encryption_public_key,
              transport_public_key, grant_epoch, status, authorized_at, revoked_at,
              revocation_epoch, revision, hlc)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(node_id) DO UPDATE SET
                key_fingerprint = excluded.key_fingerprint,
                signing_public_key = excluded.signing_public_key,
                encryption_public_key = excluded.encryption_public_key,
                transport_public_key = excluded.transport_public_key,
                grant_epoch = excluded.grant_epoch,
                status = excluded.status,
                authorized_at = excluded.authorized_at,
                revoked_at = excluded.revoked_at,
                revocation_epoch = excluded.revocation_epoch,
                revision = excluded.revision,
                hlc = excluded.hlc",
            params![
                node.node_id,
                node.key_fingerprint,
                node.signing_public_key,
                node.encryption_public_key,
                node.transport_public_key,
                grant_epoch,
                node_status_value(node.status),
                node.authorized_at,
                node.revoked_at,
                revocation_epoch,
                revision,
                node.hlc,
            ],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn node_status(value: String) -> Result<NodeStatus, String> {
    match value.as_str() {
        "active" => Ok(NodeStatus::Active),
        "revoked" => Ok(NodeStatus::Revoked),
        _ => Err(format!("invalid authorized node status: {value}")),
    }
}

fn u64_value(value: i64, field: &str) -> Result<u64, String> {
    u64::try_from(value).map_err(|_| format!("{field} is negative"))
}

fn sqlite_i64(value: u64, field: &str) -> Result<i64, String> {
    i64::try_from(value).map_err(|_| format!("{field} exceeds SQLite INTEGER range"))
}

pub fn load_authorized_node(
    connection: &Connection,
    node_id: &str,
) -> Result<Option<AuthorizedNode>, String> {
    let row = connection
        .query_row(
            "SELECT key_fingerprint, signing_public_key, encryption_public_key,
                    transport_public_key, grant_epoch, status, authorized_at, revoked_at,
                    revocation_epoch, revision, hlc FROM authorized_nodes WHERE node_id = ?1",
            params![node_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, Option<i64>>(8)?,
                    row.get::<_, i64>(9)?,
                    row.get::<_, String>(10)?,
                ))
            },
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let Some((
        fingerprint,
        signing,
        encryption,
        transport,
        epoch,
        status,
        authorized,
        revoked,
        revocation,
        revision,
        hlc,
    )) = row
    else {
        return Ok(None);
    };
    let record = AuthorizedNode {
        node_id: node_id.to_owned(),
        key_fingerprint: fingerprint,
        signing_public_key: signing,
        encryption_public_key: encryption,
        transport_public_key: transport,
        grant_epoch: u64_value(epoch, "grant_epoch")?,
        status: node_status(status)?,
        authorized_at: authorized,
        revoked_at: revoked,
        revocation_epoch: revocation
            .map(|value| u64_value(value, "revocation_epoch"))
            .transpose()?,
        revision: u64_value(revision, "revision")?,
        hlc,
    };
    record.validate().map_err(|error| error.to_string())?;
    Ok(Some(record))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authorized_node_lookup_decodes_valid_rows_and_rejects_invalid_data() {
        let connection = Connection::open_in_memory().expect("fixture database opens");
        connection
            .execute_batch(crate::schema::CREATE_TABLES)
            .expect("authorized-node schema applies");
        connection
            .execute(
                "INSERT INTO authorized_nodes
                 (node_id, key_fingerprint, signing_public_key, encryption_public_key,
                  transport_public_key, grant_epoch, status, authorized_at, revoked_at,
                  revocation_epoch, revision, hlc)
                 VALUES ('node-a', 'fp-a', 'sign-a', 'enc-a', NULL, 2, 'active',
                         '2026-08-30T00:00:00Z', NULL, NULL, 1,
                         '2026-08-30T00:00:00.000Z:000001:node-a')",
                [],
            )
            .expect("fixture node inserts");

        let node = load_authorized_node(&connection, "node-a")
            .expect("node lookup succeeds")
            .expect("fixture node exists");
        assert_eq!(node.node_id, "node-a");
        assert_eq!(node.status, NodeStatus::Active);
        assert_eq!(node.grant_epoch, 2);
        assert!(load_authorized_node(&connection, "missing")
            .expect("missing node lookup succeeds")
            .is_none());

        connection
            .execute(
                "UPDATE authorized_nodes SET status = 'unknown' WHERE node_id = 'node-a'",
                [],
            )
            .expect("fixture status updates");
        assert_eq!(
            load_authorized_node(&connection, "node-a").expect_err("unknown status is rejected"),
            "invalid authorized node status: unknown"
        );
    }

    #[test]
    fn authorized_node_write_roundtrips_and_rejects_integer_overflow() {
        let connection = Connection::open_in_memory().expect("fixture database opens");
        connection
            .execute_batch(crate::schema::CREATE_TABLES)
            .expect("authorized-node schema applies");
        let node = AuthorizedNode {
            node_id: "node-a".to_owned(),
            key_fingerprint: "fp-a".to_owned(),
            signing_public_key: "sign-a".to_owned(),
            encryption_public_key: "enc-a".to_owned(),
            transport_public_key: None,
            grant_epoch: 2,
            status: NodeStatus::Active,
            authorized_at: "2026-08-30T00:00:00Z".to_owned(),
            revoked_at: None,
            revocation_epoch: None,
            revision: 1,
            hlc: "2026-08-30T00:00:00.000Z:000001:node-a".to_owned(),
        };

        upsert_authorized_node_record(&connection, &node).expect("node persistence succeeds");

        assert_eq!(
            load_authorized_node(&connection, "node-a").expect("node query succeeds"),
            Some(node.clone())
        );
        let overflow = AuthorizedNode {
            grant_epoch: u64::MAX,
            ..node
        };
        assert_eq!(
            upsert_authorized_node_record(&connection, &overflow)
                .expect_err("SQLite integer overflow is rejected"),
            "grant_epoch exceeds SQLite INTEGER range"
        );
    }
}
