use ark_domain::integration_replication::IntegrationConfiguration;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;

pub fn upsert_integration_configuration_record(
    connection: &Connection,
    configuration: &IntegrationConfiguration,
) -> Result<(), String> {
    let scopes =
        serde_json::to_string(&configuration.public_scopes).map_err(|error| error.to_string())?;
    let settings =
        serde_json::to_string(&configuration.public_settings).map_err(|error| error.to_string())?;
    let sync_cursor = configuration
        .sync_cursor
        .map(|value| sqlite_i64(value, "sync_cursor"))
        .transpose()?;
    let revision = sqlite_i64(configuration.revision, "revision")?;
    connection
        .execute(
            "INSERT INTO integration_configurations
             (integration_id, provider, account_subject, public_scopes_json,
              public_settings_json, enabled, sync_cursor, revision, hlc)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(integration_id) DO UPDATE SET
                provider = excluded.provider,
                account_subject = excluded.account_subject,
                public_scopes_json = excluded.public_scopes_json,
                public_settings_json = excluded.public_settings_json,
                enabled = excluded.enabled,
                sync_cursor = excluded.sync_cursor,
                revision = excluded.revision,
                hlc = excluded.hlc",
            params![
                configuration.integration_id,
                configuration.provider,
                configuration.account_subject,
                scopes,
                settings,
                configuration.enabled as i64,
                sync_cursor,
                revision,
                configuration.hlc,
            ],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn parse_sync_cursor(cursor: Option<rusqlite::types::Value>) -> Result<Option<u64>, String> {
    cursor
        .map(|value| match value {
            rusqlite::types::Value::Integer(value) => u64_value(value, "sync_cursor"),
            rusqlite::types::Value::Text(value) => value
                .parse::<u64>()
                .map_err(|_| "sync_cursor must be a non-negative integer".to_string())
                .and_then(|value| {
                    sqlite_i64(value, "sync_cursor")?;
                    Ok(value)
                }),
            _ => Err("sync_cursor must be a non-negative integer".to_string()),
        })
        .transpose()
}

fn u64_value(value: i64, field: &str) -> Result<u64, String> {
    u64::try_from(value).map_err(|_| format!("{field} is negative"))
}

fn sqlite_i64(value: u64, field: &str) -> Result<i64, String> {
    i64::try_from(value).map_err(|_| format!("{field} exceeds SQLite INTEGER range"))
}

pub fn load_integration_configuration(
    connection: &Connection,
    integration_id: &str,
) -> Result<Option<IntegrationConfiguration>, String> {
    let row = connection
        .query_row(
            "SELECT provider, account_subject, public_scopes_json,
                    public_settings_json, enabled, sync_cursor, revision, hlc
             FROM integration_configurations WHERE integration_id = ?1",
            params![integration_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)? != 0,
                    row.get::<_, Option<rusqlite::types::Value>>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, String>(7)?,
                ))
            },
        )
        .optional()
        .map_err(|error| error.to_string())?;
    let Some((provider, subject, scopes, settings, enabled, cursor, revision, hlc)) = row else {
        return Ok(None);
    };
    let sync_cursor = match parse_sync_cursor(cursor) {
        Ok(value) => value,
        Err(_) => {
            connection
                .execute(
                    "UPDATE integration_configurations SET sync_cursor = NULL
                     WHERE integration_id = ?1",
                    params![integration_id],
                )
                .map_err(|error| error.to_string())?;
            None
        }
    };
    let record = IntegrationConfiguration {
        integration_id: integration_id.to_owned(),
        provider,
        account_subject: subject,
        public_scopes: serde_json::from_str(&scopes).map_err(|error| error.to_string())?,
        public_settings: serde_json::from_str::<Value>(&settings)
            .map_err(|error| error.to_string())?,
        enabled,
        sync_cursor,
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
    fn configuration_lookup_scrubs_opaque_cursor_and_keeps_numeric_cursor() {
        let connection = Connection::open_in_memory().expect("fixture database opens");
        connection
            .execute_batch(crate::schema::CREATE_TABLES)
            .expect("integration configuration schema applies");
        connection
            .execute_batch(
                "INSERT INTO integration_configurations
                 (integration_id, provider, account_subject, public_scopes_json,
                  public_settings_json, enabled, sync_cursor, revision, hlc)
                 VALUES
                  ('opaque', 'provider', 'account-a', '[]', '{}', 1, 'bearer-token', 1,
                   '2026-08-30T00:00:00.000Z:000001:node-a'),
                  ('numeric', 'provider', 'account-b', '[]', '{}', 1, '42', 1,
                   '2026-08-30T00:00:00.000Z:000001:node-a')",
            )
            .expect("fixture configurations insert");

        assert_eq!(
            load_integration_configuration(&connection, "opaque")
                .expect("opaque configuration loads")
                .expect("opaque configuration exists")
                .sync_cursor,
            None
        );
        let persisted_cursor: Option<String> = connection
            .query_row(
                "SELECT sync_cursor FROM integration_configurations
                 WHERE integration_id = 'opaque'",
                [],
                |row| row.get(0),
            )
            .expect("scrubbed cursor reads");
        assert_eq!(persisted_cursor, None);
        assert_eq!(
            load_integration_configuration(&connection, "numeric")
                .expect("numeric configuration loads")
                .expect("numeric configuration exists")
                .sync_cursor,
            Some(42)
        );
    }

    #[test]
    fn configuration_write_roundtrips_public_fields_and_cursor() {
        let connection = Connection::open_in_memory().expect("fixture database opens");
        connection
            .execute_batch(crate::schema::CREATE_TABLES)
            .expect("integration configuration schema applies");
        let configuration = IntegrationConfiguration {
            integration_id: "app".to_owned(),
            provider: "provider".to_owned(),
            account_subject: "account".to_owned(),
            public_scopes: vec!["read".to_owned()],
            public_settings: serde_json::json!({"region": "local"}),
            enabled: true,
            sync_cursor: Some(42),
            revision: 3,
            hlc: "2026-10-18T00:00:00.000Z:000003:device".to_owned(),
        };

        upsert_integration_configuration_record(&connection, &configuration)
            .expect("configuration persistence succeeds");

        assert_eq!(
            load_integration_configuration(&connection, "app")
                .expect("configuration query succeeds"),
            Some(configuration)
        );
    }
}
