use ark_domain::integration_replication::IntegrationRefreshLease;
use rusqlite::{params, Connection, OptionalExtension};

fn u64_value(value: i64, field: &str) -> Result<u64, String> {
    u64::try_from(value).map_err(|_| format!("{field} is negative"))
}

fn sqlite_i64(value: u64, field: &str) -> Result<i64, String> {
    i64::try_from(value).map_err(|_| format!("{field} exceeds SQLite INTEGER range"))
}

pub fn upsert_integration_refresh_lease_record(
    connection: &Connection,
    lease: &IntegrationRefreshLease,
) -> Result<(), String> {
    let generation = sqlite_i64(lease.credential_generation, "credential_generation")?;
    let fence = sqlite_i64(lease.fencing_token, "fencing_token")?;
    let issued = sqlite_i64(lease.issued_at_ms, "issued_at_ms")?;
    let expires = sqlite_i64(lease.expires_at_ms, "expires_at_ms")?;
    connection
        .execute(
            "INSERT INTO integration_refresh_leases
             (integration_id, credential_generation, holder_node_id, fencing_token,
              issued_at_ms, expires_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(integration_id) DO UPDATE SET
                credential_generation = excluded.credential_generation,
                holder_node_id = excluded.holder_node_id,
                fencing_token = excluded.fencing_token,
                issued_at_ms = excluded.issued_at_ms,
                expires_at_ms = excluded.expires_at_ms",
            params![
                lease.integration_id,
                generation,
                lease.holder_node_id,
                fence,
                issued,
                expires,
            ],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn load_integration_refresh_lease(
    connection: &Connection,
    integration_id: &str,
) -> Result<Option<IntegrationRefreshLease>, String> {
    let row = connection
        .query_row(
            "SELECT credential_generation, holder_node_id, fencing_token,
                    issued_at_ms, expires_at_ms FROM integration_refresh_leases
             WHERE integration_id = ?1",
            params![integration_id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            },
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let Some((generation, holder, fence, issued, expires)) = row else {
        return Ok(None);
    };
    let lease = IntegrationRefreshLease {
        integration_id: integration_id.to_owned(),
        credential_generation: u64_value(generation, "credential_generation")?,
        holder_node_id: holder,
        fencing_token: u64_value(fence, "fencing_token")?,
        issued_at_ms: u64_value(issued, "issued_at_ms")?,
        expires_at_ms: u64_value(expires, "expires_at_ms")?,
    };
    lease.validate().map_err(|error| error.to_string())?;
    Ok(Some(lease))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_lease_lookup_decodes_valid_rows_and_rejects_negative_values() {
        let connection = Connection::open_in_memory().expect("fixture database opens");
        connection
            .execute_batch(crate::schema::CREATE_TABLES)
            .expect("refresh-lease schema applies");
        connection
            .execute(
                "INSERT INTO integration_refresh_leases
                 (integration_id, credential_generation, holder_node_id, fencing_token,
                  issued_at_ms, expires_at_ms)
                 VALUES ('app', 2, 'device', 3, 1000, 2000)",
                [],
            )
            .expect("fixture lease inserts");

        let lease = load_integration_refresh_lease(&connection, "app")
            .expect("lease lookup succeeds")
            .expect("fixture lease exists");
        assert_eq!(lease.integration_id, "app");
        assert_eq!(lease.holder_node_id, "device");
        assert_eq!(lease.credential_generation, 2);
        assert_eq!(lease.fencing_token, 3);
        assert!(load_integration_refresh_lease(&connection, "missing")
            .expect("missing lease lookup succeeds")
            .is_none());

        connection
            .execute(
                "UPDATE integration_refresh_leases SET fencing_token = -1
                 WHERE integration_id = 'app'",
                [],
            )
            .expect("fixture fence updates");
        assert_eq!(
            load_integration_refresh_lease(&connection, "app")
                .expect_err("negative fence is rejected"),
            "fencing_token is negative"
        );
    }

    #[test]
    fn refresh_lease_write_roundtrips_and_rejects_integer_overflow() {
        let connection = Connection::open_in_memory().expect("fixture database opens");
        connection
            .execute_batch(crate::schema::CREATE_TABLES)
            .expect("refresh-lease schema applies");
        let lease = IntegrationRefreshLease {
            integration_id: "app".to_owned(),
            credential_generation: 2,
            holder_node_id: "device".to_owned(),
            fencing_token: 3,
            issued_at_ms: 1000,
            expires_at_ms: 2000,
        };

        upsert_integration_refresh_lease_record(&connection, &lease)
            .expect("lease persistence succeeds");

        assert_eq!(
            load_integration_refresh_lease(&connection, "app").expect("lease query succeeds"),
            Some(lease.clone())
        );
        let overflow = IntegrationRefreshLease {
            credential_generation: u64::MAX,
            ..lease
        };
        assert_eq!(
            upsert_integration_refresh_lease_record(&connection, &overflow)
                .expect_err("SQLite integer overflow is rejected"),
            "credential_generation exceeds SQLite INTEGER range"
        );
    }
}
