//! Pure Phase 3 compatibility facade. No persistence, clocks, events, or registry mutation.
mod activity;
mod content;
pub mod planning;
mod shared;

use serde_json::Value;
pub use shared::{CompatibilityError, LegacyRecord, LocalState, MappedRecord, Quarantine};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalIdentity {
    pub type_id: String,
    pub type_version: String,
}

impl CanonicalIdentity {
    pub fn new(type_id: impl Into<String>, type_version: impl Into<String>) -> Self {
        Self {
            type_id: type_id.into(),
            type_version: type_version.into(),
        }
    }
}

pub struct LegacySource<'a> {
    pub source_kind: &'a str,
    pub source_id: &'a str,
    pub raw_json: &'a [u8],
}

pub struct MappingContext {
    pub existing_object_ids: std::collections::BTreeMap<String, CanonicalIdentity>,
    pub existing_links: Vec<crate::types::ObjectLink>,
}

/// Map a stored source while retaining its identity and exact bytes for retry/quarantine.
pub fn map_legacy_with_context(
    source: LegacySource<'_>,
    context: &MappingContext,
) -> Result<MappedRecord, CompatibilityError> {
    let record: LegacyRecord =
        serde_json::from_slice(source.raw_json).map_err(|_| CompatibilityError::MalformedJson {
            source_kind: source.source_kind.into(),
            source_id: source.source_id.into(),
            pointer: "".into(),
            raw_source: source.raw_json.to_vec(),
        })?;
    let mut mapped =
        map_value(record.clone(), Some(source.raw_json.to_vec())).map_err(|failure| {
            enrich(
                failure,
                source.source_kind,
                source.source_id,
                source.raw_json,
            )
        })?;

    let fields: &[(&str, &str, &str)] = match record.legacy_type_id.as_str() {
        "book_obj" => &[("cover_image", "coverImage", "cover-image")],
        "game_obj" => &[
            ("cover_image", "coverImage", "cover-image"),
            ("background_image", "backgroundImage", "background-image"),
        ],
        _ => &[],
    };
    for (legacy_key, output_key, link_type) in fields {
        if let Some(target) = record
            .props
            .get(*legacy_key)
            .or_else(|| record.props.get(*output_key))
            .and_then(Value::as_str)
        {
            if context
                .existing_object_ids
                .get(target)
                .is_some_and(|identity| {
                    identity.type_id == "com.kosmos.image" && identity.type_version == "1.0.0"
                })
            {
                add_link(
                    &mut mapped.links,
                    &record.id,
                    target,
                    link_type,
                    &record.updated_at,
                );
                for quarantine in &mut mapped.quarantine {
                    if let Some(fields) = quarantine.fields_json.as_object_mut() {
                        fields.remove(*output_key);
                    }
                }
                mapped.quarantine.retain(|q| {
                    q.fields_json
                        .as_object()
                        .is_some_and(|fields| !fields.is_empty())
                });
            }
        }
    }
    for link in &mapped.links {
        let expected_type = expected_target_type(&record, link);
        if !context
            .existing_object_ids
            .get(&link.target_object_id)
            .is_some_and(|identity| {
                expected_type == Some(identity.type_id.as_str()) && identity.type_version == "1.0.0"
            })
        {
            let pointer = relation_pointer(&record, link);
            return Err(enrich(
                shared::CompatFailure::InvalidField { pointer },
                source.source_kind,
                source.source_id,
                source.raw_json,
            ));
        }
    }
    for existing in &context.existing_links {
        if let Some(link) = mapped.links.iter().find(|link| link.id == existing.id) {
            if link.source_object_id != existing.source_object_id
                || link.link_type != existing.link_type
                || link.target_object_id != existing.target_object_id
            {
                return Err(enrich(
                    shared::CompatFailure::ConflictingFields {
                        pointer: "/links".into(),
                    },
                    source.source_kind,
                    source.source_id,
                    source.raw_json,
                ));
            }
        }
    }
    for existing in &context.existing_links {
        if let Some(link) = mapped.links.iter_mut().find(|link| {
            link.source_object_id == existing.source_object_id
                && link.link_type == existing.link_type
                && link.target_object_id == existing.target_object_id
        }) {
            link.id = existing.id.clone();
        }
    }
    mapped.links.sort_by(|a, b| {
        (&a.link_type, &a.target_object_id, &a.id).cmp(&(&b.link_type, &b.target_object_id, &b.id))
    });
    Ok(mapped)
}

fn expected_target_type<'a>(
    record: &LegacyRecord,
    link: &crate::types::ObjectLink,
) -> Option<&'a str> {
    let source_type = shared::canonical_id(&record.legacy_type_id)?;
    Some(match link.link_type.as_str() {
        "project" => "com.kosmos.project",
        "tag" => "com.kosmos.tag",
        "task" | "for-task" => "com.kosmos.task",
        "note" | "source-note" => "com.kosmos.note",
        "person" | "author-person" => "com.kosmos.person",
        "photo" | "cover-image" | "background-image" => "com.kosmos.image",
        "related" => source_type,
        _ => return None,
    })
}

fn relation_pointer(record: &LegacyRecord, link: &crate::types::ObjectLink) -> String {
    let candidates: &[&str] = match link.link_type.as_str() {
        "project" => &["projectId", "project_id"],
        "tag" => &["tagIds", "tag_ids"],
        "related" => &[
            "relatedIds",
            "related_ids",
            "relatedNotes",
            "related_notes",
            "related",
        ],
        "task" => &["taskIds", "task_ids", "taskId", "task_id"],
        "for-task" => &["taskId", "task_id"],
        "note" => &["noteIds", "note_ids"],
        "photo" => &["photoId", "photo_id"],
        "person" => &["personId", "person_id"],
        _ => &[],
    };
    record
        .props
        .as_object()
        .and_then(|props| {
            candidates.iter().find(|key| {
                props.get(**key).is_some_and(|value| {
                    value.as_str() == Some(link.target_object_id.as_str())
                        || value.as_array().is_some_and(|values| {
                            values
                                .iter()
                                .any(|item| item.as_str() == Some(link.target_object_id.as_str()))
                        })
                })
            })
        })
        .map_or_else(|| "/links".into(), |key| format!("/props/{key}"))
}

fn add_link(
    links: &mut Vec<crate::types::ObjectLink>,
    source: &str,
    target: &str,
    link_type: &str,
    created_at: &str,
) {
    use sha2::Digest;
    let mut h = sha2::Sha256::new();
    h.update(format!("kosmos-link-v1\0{source}\0{link_type}\0{target}").as_bytes());
    links.push(crate::types::ObjectLink {
        id: format!("lnk*{:x}", h.finalize()),
        source_object_id: source.into(),
        target_object_id: target.into(),
        link_type: link_type.into(),
        created_at: created_at.into(),
    });
}

/// Parse the stored source bytes at the compatibility boundary. The bytes are never rewritten.
pub fn map_legacy_source(source: LegacySource<'_>) -> Result<MappedRecord, CompatibilityError> {
    let record: LegacyRecord =
        serde_json::from_slice(source.raw_json).map_err(|_| CompatibilityError::MalformedJson {
            source_kind: source.source_kind.into(),
            source_id: source.source_id.into(),
            pointer: "".into(),
            raw_source: source.raw_json.to_vec(),
        })?;
    map_value(record, Some(source.raw_json.to_vec())).map_err(|failure| {
        enrich(
            failure,
            source.source_kind,
            source.source_id,
            source.raw_json,
        )
    })
}

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
            &mut out,
            &mut consumed,
            &mut links,
            &mut local,
            &mut quarantine,
            &record.id,
            &record.updated_at,
        )?,
        _ => {
            return Err(shared::CompatFailure::UnsupportedLegacyValue {
                pointer: "/legacy_type_id".into(),
            })
        }
    }
    shared::extensions(props, &mut out, &consumed, &mut local, &mut quarantine)?;
    let object = crate::types::ArkObject {
        id: record.id,
        type_id: canonical.into(),
        type_version: "1.0.0".into(),
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
