

fn enrich(
    failure: shared::CompatFailure,
    source_kind: &str,
    source_id: &str,
    raw_source: &[u8],
) -> CompatibilityError {
    let (pointer, kind) = match failure {
        shared::CompatFailure::MalformedJson { pointer } => (pointer, 0),
        shared::CompatFailure::InvalidField { pointer } => (pointer, 1),
        shared::CompatFailure::ConflictingFields { pointer } => (pointer, 2),
        shared::CompatFailure::UnsupportedLegacyValue { pointer } => (pointer, 3),
        shared::CompatFailure::SecretField { pointer } => (pointer, 4),
        shared::CompatFailure::DataLossRisk { pointer } => (pointer, 5),
    };
    match kind {
        0 => CompatibilityError::MalformedJson {
            source_kind: source_kind.into(),
            source_id: source_id.into(),
            pointer,
            raw_source: raw_source.to_vec(),
        },
        1 => CompatibilityError::InvalidField {
            source_kind: source_kind.into(),
            source_id: source_id.into(),
            pointer,
            raw_source: raw_source.to_vec(),
        },
        2 => CompatibilityError::ConflictingFields {
            source_kind: source_kind.into(),
            source_id: source_id.into(),
            pointer,
            raw_source: raw_source.to_vec(),
        },
        3 => CompatibilityError::UnsupportedLegacyValue {
            source_kind: source_kind.into(),
            source_id: source_id.into(),
            pointer,
            raw_source: raw_source.to_vec(),
        },
        4 => CompatibilityError::SecretField {
            source_kind: source_kind.into(),
            source_id: source_id.into(),
            pointer,
            raw_source: raw_source.to_vec(),
        },
        _ => CompatibilityError::DataLossRisk {
            source_kind: source_kind.into(),
            source_id: source_id.into(),
            pointer,
            raw_source: raw_source.to_vec(),
        },
    }
}

fn map_value(
    record: LegacyRecord,
    raw_source: Option<Vec<u8>>,
) -> Result<MappedRecord, shared::CompatFailure> {
    let canonical = shared::canonical_id(&record.legacy_type_id).ok_or_else(|| {
        shared::CompatFailure::UnsupportedLegacyValue {
            pointer: "/legacy_type_id".into(),
        }
    })?;
    let props = record
        .props
        .as_object()
        .ok_or_else(|| shared::malformed("/props"))?;
    let rich = matches!(
        record.legacy_type_id.as_str(),
        "note_obj"
            | "task_obj"
            | "project_obj"
            | "tag_obj"
            | "person_obj"
            | "book_obj"
            | "time_entry_obj"
    );
    if !record.content.is_object() {
        return Err(shared::invalid("/content"));
    }
    shared::reject_secrets(props, "/props")?;
    let mut out = serde_json::Map::new();
    let mut local = serde_json::Map::new();
    let mut quarantine = serde_json::Map::new();
    let mut links = Vec::new();
    let mut consumed = std::collections::BTreeSet::new();
    match record.legacy_type_id.as_str() {
        "task_obj" => planning::task(
            props,
            &mut out,
            &mut consumed,
            &mut links,
            &record.id,
            &record.updated_at,
        )?,
        "project_obj" => planning::project(
            props,
            &mut out,
            &mut consumed,
            &mut links,
            &record.id,
            &record.updated_at,
        )?,
        "tag_obj" => planning::tag(
            props,
            &mut out,
            &mut consumed,
            &mut links,
            &record.id,
            &record.updated_at,
        )?,
        "note_obj" => content::note(
            props,
            &mut out,
            &mut consumed,
            &mut links,
            &record.id,
            &record.updated_at,
        )?,
        "person_obj" => content::person(
            props,
            &mut out,
            &mut consumed,
            &mut links,
            &record.id,
            &record.updated_at,
        )?,
        "image_obj" => content::image(props, &mut out, &mut consumed, &mut local, &mut quarantine)?,
        "book_obj" => content::book(
            props,
            &mut out,
            &mut consumed,
            &mut links,
            &mut quarantine,
            &record.id,
            &record.updated_at,
        )?,
        "time_entry_obj" => activity::time_entry(
            props,
            &mut out,
            &mut consumed,
            &mut links,
            &mut quarantine,
            &record.id,
            &record.updated_at,
        )?,
        "game_obj" => activity::game(
            props,
            shared::CompatCtx {
                out: &mut out,
                consumed: &mut consumed,
                links: &mut links,
                local: &mut local,
                quarantine: &mut quarantine,
                id: &record.id,
                at: &record.updated_at,
            },
        )?,
        _ => {
            return Err(shared::CompatFailure::UnsupportedLegacyValue {
                pointer: "/legacy_type_id".into(),
            })
        }
    }
    shared::extensions(props, &mut out, &consumed, &mut local, &mut quarantine)?;
    // Mapped objects claim the newest registered version of their type; a
    // schema bump must not strand new writes on the superseded contract.
    let version = crate::canonical_types::definitions::current_canonical_version(canonical)
        .ok()
        .flatten()
        .ok_or_else(|| shared::CompatFailure::DataLossRisk {
            pointer: "/type_id".into(),
        })?;
    let registration = crate::canonical_types::definitions::canonical_type_registrations()
        .map_err(|_| shared::CompatFailure::DataLossRisk {
            pointer: "/".into(),
        })?
        .into_iter()
        .find(|r| r.type_id == canonical && r.version == version)
        .ok_or_else(|| shared::CompatFailure::DataLossRisk {
            pointer: "/type_id".into(),
        })?;
    // Legacy day fields (`scheduled_date`, `deadline`) may carry RFC 3339
    // stamps; the day-field contract stores bare dates, so coerce here — the
    // validation below still rejects genuinely malformed values.
    let schema: Value =
        serde_json::from_str(&registration.schema_json).map_err(|_| {
            shared::CompatFailure::DataLossRisk {
                pointer: "/schema".into(),
            }
        })?;
    crate::canonical_types::normalize::day_props(&schema, &mut out);
    let object = crate::types::ArkObject {
        id: record.id,
        type_id: canonical.into(),
        type_version: version,
        title: record.title,
        content_json: record.content,
        props_json: Value::Object(out),
        created_at: record.created_at,
        updated_at: record.updated_at,
        deleted_at: record.deleted_at,
    };
    shared::validate(&object).map_err(|e| match e {
        shared::CompatFailure::InvalidField { pointer }
            if rich && pointer.starts_with("/content") =>
        {
            shared::CompatFailure::DataLossRisk { pointer }
        }
        other => other,
    })?;
    links.sort_by(|a, b| {
        (&a.link_type, &a.target_object_id, &a.id).cmp(&(&b.link_type, &b.target_object_id, &b.id))
    });
    links.dedup_by(|a, b| {
        a.source_object_id == b.source_object_id
            && a.link_type == b.link_type
            && a.target_object_id == b.target_object_id
    });
    Ok(MappedRecord {
        object,
        links,
        local_state: shared::bundle(local),
        quarantine: shared::qbundle(quarantine),
        raw_source,
    })
}
