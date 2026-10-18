use ark_domain::integration_replication::{AuthorizedNode, NodeStatus};
use rusqlite::{params, Connection, OptionalExtension};

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
}
