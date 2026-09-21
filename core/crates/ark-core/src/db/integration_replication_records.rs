use crate::integration_replication::{AuthorizedNode, IntegrationConfiguration, NodeStatus};
fn node_status_value(status: NodeStatus) -> &'static str {
    match status {
        NodeStatus::Active => "active",
        NodeStatus::Revoked => "revoked",
    }
}
fn node_status(value: String) -> Result<NodeStatus, String> {
    match value.as_str() {
        "active" => Ok(NodeStatus::Active),
        "revoked" => Ok(NodeStatus::Revoked),
        _ => Err(format!("invalid authorized node status: {value}")),
    }
}
pub fn upsert_integration_configuration(
    conn: &Connection,
    configuration: &IntegrationConfiguration,
    device_id: &str,
) -> Result<(), String> {
    configuration.validate().map_err(|e| e.to_string())?;
    let scopes = json(&configuration.public_scopes)?;
    let settings = json(&configuration.public_settings)?;
    let sync_cursor = configuration
        .sync_cursor
        .map(|value| sqlite_i64(value, "sync_cursor"))
        .transpose()?;
    let revision = sqlite_i64(configuration.revision, "revision")?;
    persist_with_vector(
        conn,
        "integration_configuration",
        &configuration.integration_id,
        device_id,
        || {
            if let Some((provider, subject, hlc)) = conn
                .query_row(
                    "SELECT provider, account_subject, hlc FROM integration_configurations
                     WHERE integration_id = ?1",
                    params![configuration.integration_id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?)),
                )
                .optional()
                .map_err(storage)?
            {
                if provider != configuration.provider || subject != configuration.account_subject {
                    return Err(crate::integration_replication::IntegrationContractError::Mismatch {
                        field: "provider_account_identity",
                    }
                    .to_string());
                }
                require_newer_hlc(&configuration.hlc, &hlc)?;
            }
            conn.execute(
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
            .map_err(storage)?;
            Ok(())
        },
    )
}
pub fn load_integration_configuration(
    conn: &Connection,
    integration_id: &str,
) -> Result<Option<IntegrationConfiguration>, String> {
    let row = conn
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
        .map_err(storage)?;
    let Some((provider, subject, scopes, settings, enabled, cursor, revision, hlc)) = row else {
        return Ok(None);
    };
    let sync_cursor = match parse_sync_cursor(cursor) {
        Ok(value) => value,
        Err(_) => {
            // Legacy databases may contain an opaque provider cursor here.
            // Remove it before returning any configuration or exporting a
            // replication frame; opaque provider state belongs in ciphertext.
            conn.execute(
                "UPDATE integration_configurations SET sync_cursor = NULL
                 WHERE integration_id = ?1",
                params![integration_id],
            )
            .map_err(storage)?;
            None
        }
    };
    let record = IntegrationConfiguration {
        integration_id: integration_id.to_string(),
        provider,
        account_subject: subject,
        public_scopes: parse(&scopes)?,
        public_settings: parse::<Value>(&settings)?,
        enabled,
        sync_cursor,
        revision: u64_value(revision, "revision")?,
        hlc,
    };
    record.validate().map_err(|e| e.to_string())?;
    Ok(Some(record))
}
fn parse_sync_cursor(
    cursor: Option<rusqlite::types::Value>,
) -> Result<Option<u64>, String> {
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

pub fn upsert_authorized_node(
    conn: &Connection,
    node: &AuthorizedNode,
    device_id: &str,
) -> Result<(), String> {
    node.validate().map_err(|e| e.to_string())?;
    let grant_epoch = sqlite_i64(node.grant_epoch, "grant_epoch")?;
    let revocation_epoch = node
        .revocation_epoch
        .map(|v| sqlite_i64(v, "revocation_epoch"))
        .transpose()?;
    let revision = sqlite_i64(node.revision, "revision")?;
    persist_with_vector(conn, "authorized_node", &node.node_id, device_id, || {
        let current = conn
            .query_row(
                "SELECT grant_epoch, status, key_fingerprint, signing_public_key,
                        encryption_public_key, transport_public_key, hlc
                 FROM authorized_nodes WHERE node_id = ?1",
                params![node.node_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, String>(6)?,
                    ))
                },
            )
            .optional()
            .map_err(storage)?;
        if let Some((old_epoch, old_status, fingerprint, signing, encryption, transport, hlc)) = current {
            if grant_epoch < old_epoch {
                return Err(crate::integration_replication::IntegrationContractError::NonMonotonic {
                    field: "grant_epoch",
                }
                .to_string());
            }
            if grant_epoch == old_epoch {
                if old_status == "revoked" && node.status == NodeStatus::Active {
                    return Err(crate::integration_replication::IntegrationContractError::RevokedNode.to_string());
                }
                if fingerprint != node.key_fingerprint
                    || signing != node.signing_public_key
                    || encryption != node.encryption_public_key
                    || transport != node.transport_public_key
                {
                    return Err(crate::integration_replication::IntegrationContractError::Mismatch {
                        field: "node_key_epoch",
                    }
                    .to_string());
                }
                require_newer_hlc(&node.hlc, &hlc)?;
            }
        }
        conn.execute(
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
        .map_err(storage)?;
        Ok(())
    })
}

pub fn load_authorized_node(
    conn: &Connection,
    node_id: &str,
) -> Result<Option<AuthorizedNode>, String> {
    let row = conn
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
        .map_err(storage)?;
    let Some((fingerprint, signing, encryption, transport, epoch, status, authorized, revoked,
        revocation, revision, hlc)) = row else { return Ok(None); };
    let record = AuthorizedNode {
        node_id: node_id.to_string(),
        key_fingerprint: fingerprint,
        signing_public_key: signing,
        encryption_public_key: encryption,
        transport_public_key: transport,
        grant_epoch: u64_value(epoch, "grant_epoch")?,
        status: node_status(status)?,
        authorized_at: authorized,
        revoked_at: revoked,
        revocation_epoch: revocation.map(|v| u64_value(v, "revocation_epoch")).transpose()?,
        revision: u64_value(revision, "revision")?,
        hlc,
    };
    record.validate().map_err(|e| e.to_string())?;
    Ok(Some(record))
}
