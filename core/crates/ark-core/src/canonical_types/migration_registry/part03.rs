
fn archive_and_promote(
    conn: &Connection,
    registration: &TypeRegistration,
    legacy: &str,
) -> Result<ArchiveEvidence, RegistryError> {
    let evidence = archive_evidence(conn, legacy, &registration.type_id)?;
    match existing_archive(conn, &evidence)? {
        Some(true) => {}
        Some(false) => {
            return Err(RegistryError::ArchiveConflict {
                type_id: legacy.into(),
            })
        }
        None => {
            conn.execute(concat!("INSERT INTO legacy_type_definition_archive(contract_version,legacy_type_id,","canonical_type_id,summary_json,versions_json,inbound_aliases_json,","source_hash,archived_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)"), params![CONTRACT_VERSION, evidence.legacy_type_id, evidence.canonical_type_id, evidence.summary_json, evidence.versions_json, evidence.inbound_aliases_json, evidence.source_hash, evidence.archived_at]).map_err(storage)?;
        }
    }
    let object_count: i64 = conn
        .query_row(
            "SELECT count(*) FROM objects WHERE type_id=?1",
            [legacy],
            |row| row.get(0),
        )
        .map_err(storage)?;
    if object_count != 0 {
        return Err(RegistryError::LegacyPromotionConflict {
            type_id: legacy.into(),
        });
    }
    conn.execute(
        "UPDATE object_types SET base_type_id=?1 WHERE base_type_id=?2",
        params![registration.type_id, legacy],
    )
    .map_err(storage)?;
    conn.execute(
        "UPDATE object_type_aliases SET canonical_type_id=?1 WHERE canonical_type_id=?2",
        params![registration.type_id, legacy],
    )
    .map_err(storage)?;
    conn.execute(
        "DELETE FROM object_type_versions WHERE type_id=?1",
        [legacy],
    )
    .map_err(storage)?;
    conn.execute("DELETE FROM object_types WHERE id=?1", [legacy])
        .map_err(storage)?;
    Ok(evidence)
}

fn apply_inner(
    conn: &Connection,
    plan: &RegistryPlan,
    fail_after_archives: Option<usize>,
) -> Result<RegistryReport, RegistryError> {
    let registrations = canonical_type_registrations()
        .map_err(|detail| RegistryError::InvariantViolation { detail })?;
    ensure_archive_schema(conn)?;
    for registration in &registrations {
        install_definition(conn, registration)?;
    }
    let mut archived = 0;
    let mut promoted_aliases = Vec::new();
    for registration in &registrations {
        for alias in &registration.aliases {
            let legacy_exists = conn
                .query_row(
                    "SELECT 1 FROM object_types WHERE id=?1",
                    [alias.alias.as_str()],
                    |_| Ok(()),
                )
                .optional()
                .map_err(storage)?
                .is_some();
            if legacy_exists {
                archive_and_promote(conn, registration, &alias.alias)?;
                archived += 1;
                if fail_after_archives == Some(archived) {
                    return Err(RegistryError::InjectedFailure);
                }
                promoted_aliases.push(alias.alias.clone());
            }
            let existing: Option<(String, String)> = conn
                .query_row(
                    "SELECT canonical_type_id,created_at FROM object_type_aliases WHERE alias=?1",
                    [alias.alias.as_str()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()
                .map_err(storage)?;
            match existing {
                Some((target, _)) if target != registration.type_id => {
                    return Err(RegistryError::AliasTargetConflict {
                        alias: alias.alias.clone(),
                    })
                }
                Some(_) => {}
                None => {
                    conn.execute(concat!("INSERT INTO object_type_aliases(alias,canonical_type_id,created_at) VALUES(","?1,?2,?3)"), params![alias.alias, registration.type_id, alias.created_at]).map_err(storage)?;
                }
            }
        }
    }
    Ok(RegistryReport {
        contract_version: plan.contract_version.clone(),
        installed: registrations.len(),
        archived,
        promoted_aliases,
    })
}

/// Install canonical definitions before object projection; legacy authority is archived
/// only after object rows have been rewritten to canonical identities.
pub fn prepare_registry_for_objects(conn: &Connection) -> Result<(), RegistryError> {
    ensure_archive_schema(conn)?;
    let registrations = canonical_type_registrations()
        .map_err(|detail| RegistryError::InvariantViolation { detail })?;
    for registration in &registrations {
        install_definition(conn, registration)?;
    }
    Ok(())
}

/// Idempotent post-migration step: installs canonical version rows that a
/// definition bump added after the phase3 migration completed, and bumps
/// `current_version` to the newest registered version. Existing version rows
/// are only hash-checked, never rewritten.
pub fn ensure_canonical_type_versions(conn: &Connection) -> Result<(), RegistryError> {
    let registrations = canonical_type_registrations()
        .map_err(|detail| RegistryError::InvariantViolation { detail })?;
    for registration in &registrations {
        install_definition(conn, registration)?;
    }
    Ok(())
}

/// Apply a previously read-only plan inside a savepoint. The caller retains transaction ownership.
pub fn apply_registry(
    conn: &Connection,
    plan: &RegistryPlan,
) -> Result<RegistryReport, RegistryError> {
    apply_registry_with_failure(conn, plan, None)
}

/// Test seam for proving that archive/install mutations are atomic under a late failure.
pub fn apply_registry_with_failure(
    conn: &Connection,
    plan: &RegistryPlan,
    fail_after_archives: Option<usize>,
) -> Result<RegistryReport, RegistryError> {
    conn.execute_batch("SAVEPOINT phase3_registry")
        .map_err(storage)?;
    let result = apply_inner(conn, plan, fail_after_archives);
    match result {
        Ok(report) => {
            conn.execute_batch("RELEASE SAVEPOINT phase3_registry")
                .map_err(storage)?;
            Ok(report)
        }
        Err(error) => {
            let _ = conn.execute_batch(
                "ROLLBACK TO SAVEPOINT phase3_registry; RELEASE SAVEPOINT phase3_registry",
            );
            Err(error)
        }
    }
}
