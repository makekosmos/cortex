use ark_domain::integration_replication::{GrantStatus, IntegrationNodeGrant};
use rusqlite::{params, Connection, OptionalExtension};

fn status_value(status: GrantStatus) -> &'static str {
    match status {
        GrantStatus::Active => "active",
        GrantStatus::Revoked => "revoked",
    }
}

pub fn upsert_integration_node_grant_record(
    connection: &Connection,
    grant: &IntegrationNodeGrant,
) -> Result<(), String> {
    let epoch = i64::try_from(grant.grant_epoch)
        .map_err(|_| "grant_epoch exceeds SQLite INTEGER range".to_owned())?;
    let revision = i64::try_from(grant.revision)
        .map_err(|_| "revision exceeds SQLite INTEGER range".to_owned())?;
    connection
        .execute(
            "INSERT INTO integration_node_grants
             (integration_id, node_id, node_encryption_key, grant_epoch, status,
              authorized_at, revoked_at, revision, hlc)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(integration_id, node_id) DO UPDATE SET
                node_encryption_key = excluded.node_encryption_key,
                grant_epoch = excluded.grant_epoch,
                status = excluded.status,
                authorized_at = excluded.authorized_at,
                revoked_at = excluded.revoked_at,
                revision = excluded.revision,
                hlc = excluded.hlc",
            params![
                grant.integration_id,
                grant.node_id,
                grant.node_encryption_key,
                epoch,
                status_value(grant.status),
                grant.authorized_at,
                grant.revoked_at,
                revision,
                grant.hlc,
            ],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn grant_status(value: String) -> Result<GrantStatus, String> {
    match value.as_str() {
        "active" => Ok(GrantStatus::Active),
        "revoked" => Ok(GrantStatus::Revoked),
        _ => Err(format!("invalid integration grant status: {value}")),
    }
}

pub fn load_integration_node_grant(
    connection: &Connection,
    integration_id: &str,
    node_id: &str,
) -> Result<Option<IntegrationNodeGrant>, String> {
    let row = connection
        .query_row(
            "SELECT node_encryption_key, grant_epoch, status, authorized_at,
                    revoked_at, revision, hlc FROM integration_node_grants
             WHERE integration_id = ?1 AND node_id = ?2",
            params![integration_id, node_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, String>(6)?,
                ))
            },
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let Some((encryption, epoch, state, authorized, revoked, revision, hlc)) = row else {
        return Ok(None);
    };
    let record = IntegrationNodeGrant {
        integration_id: integration_id.to_owned(),
        node_id: node_id.to_owned(),
        node_encryption_key: encryption,
        grant_epoch: u64::try_from(epoch).map_err(|_| "grant_epoch is negative".to_owned())?,
        status: grant_status(state)?,
        authorized_at: authorized,
        revoked_at: revoked,
        revision: u64::try_from(revision).map_err(|_| "revision is negative".to_owned())?,
        hlc,
    };
    record.validate().map_err(|error| error.to_string())?;
    Ok(Some(record))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grant_read_preserves_valid_record_and_rejects_invalid_status() {
        let connection = Connection::open_in_memory().expect("fixture database opens");
        connection
            .execute_batch(crate::schema::CREATE_TABLES)
            .expect("grant schema applies");
        connection
            .execute(
                "INSERT INTO integration_node_grants
                 (integration_id, node_id, node_encryption_key, grant_epoch, status,
                  authorized_at, revoked_at, revision, hlc)
                 VALUES ('app', 'device', 'public-key', 2, 'active', '2026-10-18', NULL, 3, 'hlc')",
                [],
            )
            .expect("fixture grant inserts");

        let grant = load_integration_node_grant(&connection, "app", "device")
            .expect("grant lookup succeeds")
            .expect("fixture grant exists");
        assert_eq!(grant.grant_epoch, 2);
        assert_eq!(grant.revision, 3);
        assert_eq!(grant.status, GrantStatus::Active);
        assert_eq!(grant.node_encryption_key, "public-key");
        assert!(load_integration_node_grant(&connection, "app", "unknown")
            .expect("missing grant lookup succeeds")
            .is_none());

        connection
            .execute(
                "UPDATE integration_node_grants SET status = 'unknown' WHERE node_id = 'device'",
                [],
            )
            .expect("fixture grant status updates");
        assert_eq!(
            load_integration_node_grant(&connection, "app", "device")
                .expect_err("unknown grant status is rejected"),
            "invalid integration grant status: unknown"
        );
    }

    #[test]
    fn grant_write_roundtrips_a_valid_record_and_rejects_integer_overflow() {
        let connection = Connection::open_in_memory().expect("fixture database opens");
        connection
            .execute_batch(crate::schema::CREATE_TABLES)
            .expect("grant schema applies");
        let grant = IntegrationNodeGrant {
            integration_id: "app".to_owned(),
            node_id: "device".to_owned(),
            node_encryption_key: "public-key".to_owned(),
            grant_epoch: 2,
            status: GrantStatus::Active,
            authorized_at: "2026-10-18".to_owned(),
            revoked_at: None,
            revision: 3,
            hlc: "2026-10-18T00:00:00.000Z:000003:device".to_owned(),
        };

        upsert_integration_node_grant_record(&connection, &grant)
            .expect("grant persistence succeeds");

        assert_eq!(
            load_integration_node_grant(&connection, "app", "device")
                .expect("grant query succeeds"),
            Some(grant.clone())
        );
        let overflow = IntegrationNodeGrant {
            grant_epoch: u64::MAX,
            ..grant
        };
        assert_eq!(
            upsert_integration_node_grant_record(&connection, &overflow)
                .expect_err("SQLite integer overflow is rejected"),
            "grant_epoch exceeds SQLite INTEGER range"
        );
    }
}
