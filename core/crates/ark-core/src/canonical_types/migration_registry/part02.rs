
type LegacyDefinitionRow = (
    String,
    String,
    String,
    String,
    Option<String>,
    String,
    Option<String>,
    i64,
);

fn is_generated_legacy_definition(
    conn: &Connection,
    registration: &TypeRegistration,
) -> Result<bool, RegistryError> {
    let row: Option<LegacyDefinitionRow> = conn
        .query_row(
            "SELECT current_version,schema_json,ui_schema_json,owner_kind,owner_id,status,base_type_id,system_locked FROM object_types WHERE id=?1",
            [registration.type_id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?, row.get(7)?)),
        )
        .optional()
        .map_err(storage)?;
    let Some((
        current_version,
        schema,
        ui_schema,
        owner_kind,
        owner_id,
        status,
        base_type_id,
        system_locked,
    )) = row
    else {
        return Ok(false);
    };
    let Ok((expected_version, expected_hash)) = legacy_compatibility_version(&schema, &ui_schema)
    else {
        return Ok(false);
    };
    if current_version != expected_version
        || owner_kind != "system"
        || owner_id.is_some()
        || status != "active"
        || base_type_id.is_some()
        || system_locked != 1
    {
        return Ok(false);
    }
    let version: Option<(String, String, String, String, String, String)> = conn
        .query_row(
            "SELECT schema_json,ui_schema_json,content_contract_json,relations_json,sync_policy_json,schema_hash FROM object_type_versions WHERE type_id=?1 AND version=?2",
            params![registration.type_id, current_version],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
        )
        .optional()
        .map_err(storage)?;
    let Some((version_schema, version_ui, content, relations, policy, hash)) = version else {
        return Ok(false);
    };
    Ok(
        normalized_json(&schema, "schema_json")?
            == normalized_json(&version_schema, "schema_json")?
            && normalized_json(&ui_schema, "ui_schema_json")?
                == normalized_json(&version_ui, "ui_schema_json")?
            && normalized_json(&content, "content_contract_json")? == "{}"
            && normalized_json(&relations, "relations_json")? == "[]"
            && normalized_json(&policy, "sync_policy_json")? == "{}"
            && hash == expected_hash,
    )
}

/// Read-only registry inventory and collision validation. No schema or data is changed.
pub fn preflight_registry(conn: &Connection) -> Result<RegistryPlan, RegistryError> {
    let registrations = canonical_type_registrations()
        .map_err(|detail| RegistryError::InvariantViolation { detail })?;
    let canonical_ids: Vec<_> = registrations.iter().map(|r| r.type_id.clone()).collect();
    let mut legacy_type_ids = Vec::new();
    for registration in &registrations {
        let canonical_as_alias: Option<String> = conn
            .query_row(
                "SELECT canonical_type_id FROM object_type_aliases WHERE alias=?1",
                [registration.type_id.as_str()],
                |row| row.get(0),
            )
            .optional()
            .map_err(storage)?;
        if canonical_as_alias.is_some() {
            return Err(RegistryError::CanonicalAliasCollision {
                type_id: registration.type_id.clone(),
            });
        }
        if canonical_exists(conn, registration)?
            && !is_generated_legacy_definition(conn, registration)?
        {
            let row: (String, Option<String>, String, String, i64, Option<String>, String) = conn
                .query_row(
                    "SELECT name,owner_id,current_version,status,system_locked,base_type_id,owner_kind FROM object_types WHERE id=?1",
                    [registration.type_id.as_str()],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?)),
                )
                .map_err(storage)?;
            if row.0 != registration.name
                || row.1 != registration.owner_id
                || row.2 != registration.version
                || row.3 != registration.status
                || row.5 != registration.base_type_id
                || row.6 != registration.owner_kind
            {
                return Err(RegistryError::CanonicalConflict {
                    type_id: registration.type_id.clone(),
                });
            }
            let version = registration_version(registration)?;
            let existing: Option<(String, String, String, String, String, String)> = conn
                .query_row(
                    "SELECT schema_json,ui_schema_json,content_contract_json,relations_json,sync_policy_json,schema_hash FROM object_type_versions WHERE type_id=?1 AND version=?2",
                    params![registration.type_id, registration.version],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
                )
                .optional()
                .map_err(storage)?;
            let matches = existing
                .map(|row| {
                    row.0 == version.0
                        && row.1 == version.1
                        && row.2 == version.2
                        && row.3 == version.3
                        && row.4 == version.4
                        && row.5 == registration.schema_hash
                })
                .unwrap_or(false);
            if !matches {
                return Err(RegistryError::CanonicalConflict {
                    type_id: registration.type_id.clone(),
                });
            }
        }
        for alias in &registration.aliases {
            let target: Option<String> = conn
                .query_row(
                    "SELECT canonical_type_id FROM object_type_aliases WHERE alias=?1",
                    [alias.alias.as_str()],
                    |row| row.get(0),
                )
                .optional()
                .map_err(storage)?;
            if let Some(target) = target {
                if target != registration.type_id {
                    return Err(RegistryError::AliasTargetConflict {
                        alias: alias.alias.clone(),
                    });
                }
            }
            if canonical_ids.iter().any(|id| id == &alias.alias) {
                return Err(RegistryError::CanonicalAliasCollision {
                    type_id: alias.alias.clone(),
                });
            }
            let alias_exists = conn
                .query_row(
                    "SELECT 1 FROM object_types WHERE id=?1",
                    [alias.alias.as_str()],
                    |_| Ok(()),
                )
                .optional()
                .map_err(storage)?
                .is_some();
            if alias_exists && !legacy_type_ids.contains(&alias.alias) {
                legacy_type_ids.push(alias.alias.clone());
            }
        }
        if conn
            .query_row(
                "SELECT 1 FROM object_types WHERE id=?1",
                [registration.aliases[0].alias.as_str()],
                |_| Ok(()),
            )
            .optional()
            .map_err(storage)?
            .is_some()
        {
            let legacy = &registration.aliases[0].alias;
            let evidence = archive_evidence(conn, legacy, &registration.type_id)?;
            if let Some(false) = existing_archive(conn, &evidence)? {
                return Err(RegistryError::ArchiveConflict {
                    type_id: legacy.clone(),
                });
            }
        }
    }
    legacy_type_ids.sort();
    Ok(RegistryPlan {
        contract_version: CONTRACT_VERSION.into(),
        canonical_type_ids: canonical_ids,
        legacy_type_ids,
    })
}

fn ensure_archive_schema(conn: &Connection) -> Result<(), RegistryError> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS legacy_type_definition_archive (contract_version TEXT NOT NULL, legacy_type_id TEXT NOT NULL, canonical_type_id TEXT NOT NULL, summary_json TEXT NOT NULL, versions_json TEXT NOT NULL, inbound_aliases_json TEXT NOT NULL, source_hash TEXT NOT NULL, archived_at TEXT NOT NULL, PRIMARY KEY(contract_version,legacy_type_id)); CREATE INDEX IF NOT EXISTS idx_legacy_type_definition_archive_canonical ON legacy_type_definition_archive(canonical_type_id);").map_err(storage)
}

fn install_definition(
    conn: &Connection,
    registration: &TypeRegistration,
) -> Result<(), RegistryError> {
    let exists = canonical_exists(conn, registration)?;
    if !exists {
        conn.execute("INSERT INTO object_types(id,name,schema_json,ui_schema_json,created_at,updated_at,system_locked,owner_kind,owner_id,current_version,status,base_type_id) VALUES(?1,?2,?3,?4,?5,?5,1,?6,?7,?8,?9,?10)", params![registration.type_id, registration.name, registration.schema_json, registration.ui_schema_json, registration.created_at, registration.owner_kind, registration.owner_id, registration.version, registration.status, registration.base_type_id]).map_err(storage)?;
    } else {
        if is_generated_legacy_definition(conn, registration)? {
            let evidence = archive_evidence(conn, &registration.type_id, &registration.type_id)?;
            match existing_archive(conn, &evidence)? {
                Some(true) => {}
                Some(false) => {
                    return Err(RegistryError::ArchiveConflict {
                        type_id: registration.type_id.clone(),
                    })
                }
                None => {
                    conn.execute("INSERT INTO legacy_type_definition_archive(contract_version,legacy_type_id,canonical_type_id,summary_json,versions_json,inbound_aliases_json,source_hash,archived_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)", params![CONTRACT_VERSION, evidence.legacy_type_id, evidence.canonical_type_id, evidence.summary_json, evidence.versions_json, evidence.inbound_aliases_json, evidence.source_hash, evidence.archived_at]).map_err(storage)?;
                }
            }
            conn.execute("UPDATE object_types SET name=?2,schema_json=?3,ui_schema_json=?4,created_at=?5,updated_at=?5,system_locked=1,owner_kind=?6,owner_id=?7,current_version=?8,status=?9,base_type_id=?10 WHERE id=?1", params![registration.type_id, registration.name, registration.schema_json, registration.ui_schema_json, registration.created_at, registration.owner_kind, registration.owner_id, registration.version, registration.status, registration.base_type_id]).map_err(storage)?;
        } else {
            conn.execute(
                "UPDATE object_types SET system_locked=1 WHERE id=?1",
                [registration.type_id.as_str()],
            )
            .map_err(storage)?;
        }
    }
    let version = registration_version(registration)?;
    let version_exists: Option<(String, String, String, String, String, String)> = conn
        .query_row(
            "SELECT schema_json,ui_schema_json,content_contract_json,relations_json,sync_policy_json,schema_hash FROM object_type_versions WHERE type_id=?1 AND version=?2",
            params![registration.type_id, registration.version],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
        )
        .optional()
        .map_err(storage)?;
    if let Some(existing) = version_exists {
        if existing
            != (
                version.0.clone(),
                version.1.clone(),
                version.2.clone(),
                version.3.clone(),
                version.4.clone(),
                registration.schema_hash.clone(),
            )
        {
            return Err(RegistryError::CanonicalConflict {
                type_id: registration.type_id.clone(),
            });
        }
        return Ok(());
    }
    conn.execute("INSERT INTO object_type_versions(type_id,version,schema_json,ui_schema_json,content_contract_json,relations_json,sync_policy_json,schema_hash,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)", params![registration.type_id, registration.version, version.0, version.1, version.2, version.3, version.4, registration.schema_hash, registration.created_at]).map(|_| ()).map_err(storage)
}
