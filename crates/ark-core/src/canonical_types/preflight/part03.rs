
fn context(conn: &Connection) -> Result<MappingContext, String> {
    let regs = canonical_type_registrations()?;
    let aliases: BTreeMap<String, CanonicalIdentity> = regs
        .iter()
        .flat_map(|r| {
            r.aliases.iter().map(move |a| {
                (
                    a.alias.clone(),
                    CanonicalIdentity::new(r.type_id.clone(), r.version.clone()),
                )
            })
        })
        .collect();
    let mut ids = BTreeMap::new();
    let mut stmt = conn
        .prepare("SELECT id,type_id,type_version FROM objects")
        .map_err(|e| e.to_string())?;
    for row in stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })
        .map_err(|e| e.to_string())?
    {
        let (id, type_id, version) = row.map_err(|e| e.to_string())?;
        ids.insert(
            id,
            aliases
                .get(&type_id)
                .cloned()
                .unwrap_or_else(|| CanonicalIdentity::new(type_id, version)),
        );
    }
    Ok(MappingContext {
        existing_object_ids: ids,
        existing_links: crate::db::list_object_links(conn)?,
    })
}

pub fn preflight_phase3(conn: &Connection) -> Result<MigrationPreflightReport, String> {
    let mut blocked = Vec::new();
    let mut records = Vec::new();
    generic_inventory(conn, &mut records, &mut blocked)?;
    native_inventory(conn, &mut records)?;
    records.sort_by(|a, b| {
        (a.source_kind.name(), &a.source_id).cmp(&(b.source_kind.name(), &b.source_id))
    });
    let inventory_bytes = compact(&Value::Array(records.iter().map(|r| json!({"sourceKind":r.source_kind.name(),"sourceId":r.source_id,"sourceHash":r.source_hash})).collect()))?;
    let inventory_hash = hash_bytes(&inventory_bytes);
    let mut errors = blocked.clone();
    let ctx = context(conn)?;
    for record in &records {
        if record.canonical_bytes.is_empty() || matches!(record.source_kind, SourceKind::Native(_))
        {
            continue;
        }
        if let Err(error) = map_legacy_with_context(
            LegacySource {
                source_kind: record.source_kind.name(),
                source_id: &record.source_id,
                raw_json: &record.canonical_bytes,
            },
            &ctx,
        ) {
            errors.push(BlockedItem {
                source_kind: error.source_kind().into(),
                source_id: error.source_id().into(),
                pointer: error.pointer().into(),
                code: error.code().into(),
                raw_source: error.raw_source().to_vec(),
            });
        }
    }
    errors.sort_by(|a, b| {
        (&a.source_kind, &a.source_id, &a.pointer, &a.code).cmp(&(
            &b.source_kind,
            &b.source_id,
            &b.pointer,
            &b.code,
        ))
    });
    let mut types = Vec::new();
    for kind in [
        "note_obj",
        "task_obj",
        "project_obj",
        "tag_obj",
        "person_obj",
        "image_obj",
        "time_entry_obj",
        "game_obj",
        "book_obj",
        "todos",
        "projects",
        "areas",
        "tags",
        "headings",
    ] {
        let ids: Vec<String> = records
            .iter()
            .filter(|r| r.source_kind.name() == kind)
            .map(|r| r.source_id.clone())
            .collect();
        if !ids.is_empty() {
            types.push(PreflightType {
                type_id: kind.into(),
                source_kind: kind.into(),
                before_count: ids.len(),
                before_ids: ids,
                source_hash: hash_bytes(&compact(&json!(kind))?),
            });
        }
    }
    Ok(MigrationPreflightReport {
        contract_version: "phase3-canonical-v1".into(),
        status: if errors.is_empty() {
            "ready"
        } else {
            "blocked"
        }
        .into(),
        source_inventory_hash: inventory_hash,
        types,
        blocked,
        transformations: Vec::new(),
        errors,
    })
}
