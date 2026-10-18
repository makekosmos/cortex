use ark_domain::integration_replication::IntegrationRefreshLease;
use rusqlite::{params, Connection, OptionalExtension};

fn u64_value(value: i64, field: &str) -> Result<u64, String> {
    u64::try_from(value).map_err(|_| format!("{field} is negative"))
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
}
