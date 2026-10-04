use super::*;
use serde_json::Value;

// Pure Phase 3 compatibility facade. No persistence, clocks, events, or registry mutation.

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
                    identity.type_id == "com.kosmos.image"
                        && crate::canonical_types::definitions::is_canonical_version(
                            &identity.type_id,
                            &identity.type_version,
                        )
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
    if record.legacy_type_id == "project_obj" {
        if let Some(target) = record
            .props
            .get("area_id")
            .or_else(|| record.props.get("areaId"))
            .and_then(Value::as_str)
        {
            if context
                .existing_object_ids
                .get(target)
                .is_some_and(|identity| {
                    identity.type_id == "com.kosmos.project"
                        && crate::canonical_types::definitions::is_canonical_version(
                            &identity.type_id,
                            &identity.type_version,
                        )
                })
            {
                add_link(
                    &mut mapped.links,
                    &record.id,
                    target,
                    "related",
                    &record.updated_at,
                );
            }
        }
    }
    for link in &mapped.links {
        let expected_type = expected_target_type(&record, link);
        if !context
            .existing_object_ids
            .get(&link.target_object_id)
            .is_some_and(|identity| {
                expected_type == Some(identity.type_id.as_str())
                    && crate::canonical_types::definitions::is_canonical_version(
                        &identity.type_id,
                        &identity.type_version,
                    )
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
