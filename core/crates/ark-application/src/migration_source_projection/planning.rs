use super::inventory::{BlockedItem, SourceKind, SourceRecord};
use super::{canonical_aliases, inventory_snapshot};
use crate::canonical_compatibility::{
    map_legacy_with_context, CanonicalIdentity, LegacySource, MappedRecord, MappingContext,
};
use crate::source_snapshot::SourceSnapshot;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub const CONTRACT_VERSION: &str = "phase3-canonical-v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedItem {
    pub source_kind: String,
    pub source_kind_variant: SourceKind,
    pub source_id: String,
    pub source_hash: String,
    pub raw_source: Vec<u8>,
    pub mapped: MappedRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectPlan {
    pub contract_version: String,
    pub source_inventory_hash: String,
    pub items: Vec<PlannedItem>,
    pub blocked: Vec<BlockedItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ObjectPlanError {
    Blocked {
        source_kind: String,
        source_id: String,
        pointer: String,
        code: String,
        raw_source: Vec<u8>,
    },
    Storage(String),
    CanonicalConflict {
        source_kind: String,
        source_id: String,
    },
}

impl std::fmt::Display for ObjectPlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Blocked { .. } => "Blocked",
            Self::Storage(_) => "Storage",
            Self::CanonicalConflict { .. } => "CanonicalConflict",
        })
    }
}

impl std::error::Error for ObjectPlanError {}

fn inventory_hash(records: &[SourceRecord]) -> Result<String, ObjectPlanError> {
    let rows: Vec<Value> = records
        .iter()
        .map(|record| {
            serde_json::json!({
                "sourceKind": record.source_kind.name(),
                "sourceId": record.source_id,
                "sourceHash": record.source_hash,
            })
        })
        .collect();
    serde_json::to_vec(&rows)
        .map(|bytes| format!("{:x}", Sha256::digest(bytes)))
        .map_err(|error| ObjectPlanError::Storage(error.to_string()))
}

fn context(
    snapshot: &SourceSnapshot,
    records: &[SourceRecord],
) -> Result<MappingContext, ObjectPlanError> {
    let aliases = canonical_aliases().map_err(ObjectPlanError::Storage)?;
    let mut context = MappingContext {
        existing_object_ids: snapshot
            .object_identities
            .iter()
            .map(|identity| {
                (
                    identity.id.clone(),
                    aliases.get(&identity.type_id).cloned().unwrap_or_else(|| {
                        CanonicalIdentity::new(
                            identity.type_id.clone(),
                            identity.type_version.clone(),
                        )
                    }),
                )
            })
            .collect(),
        existing_links: snapshot.object_links.clone(),
    };
    for record in records {
        let canonical_type = match record.source_kind.name() {
            "todos" => "core.task",
            "projects" | "areas" | "headings" => "com.kosmos.project",
            "tags" => "com.kosmos.tag",
            kind => aliases
                .get(kind)
                .map_or(kind, |identity| identity.type_id.as_str()),
        };
        let version =
            ark_domain::canonical_type::definitions::current_canonical_version(canonical_type)
                .ok()
                .flatten()
                .ok_or_else(|| {
                    ObjectPlanError::Storage(format!(
                        "no canonical registration for {canonical_type}"
                    ))
                })?;
        context.existing_object_ids.insert(
            record.source_id.clone(),
            CanonicalIdentity::new(canonical_type, version),
        );
    }
    Ok(context)
}

fn compatibility_source(record: &SourceRecord) -> (String, &[u8]) {
    let kind = match &record.source_kind {
        SourceKind::Native(kind) => match kind.as_str() {
            "todos" => "task_obj",
            "projects" | "areas" | "headings" => "project_obj",
            "tags" => "tag_obj",
            other => other,
        },
        SourceKind::Legacy(kind) => kind,
    };
    (kind.to_owned(), &record.canonical_bytes)
}

fn blocked(error: &crate::canonical_compatibility::CompatibilityError) -> BlockedItem {
    BlockedItem {
        source_kind: error.source_kind().into(),
        source_id: error.source_id().into(),
        pointer: error.pointer().into(),
        code: error.code().into(),
        raw_source: error.raw_source().to_vec(),
    }
}

/// Build a deterministic migration plan from the Store-owned read snapshot.
pub fn plan_objects_from_snapshot(
    snapshot: &SourceSnapshot,
    _now: &str,
) -> Result<ObjectPlan, ObjectPlanError> {
    let records = inventory_snapshot(snapshot).map_err(ObjectPlanError::Storage)?;
    let context = context(snapshot, &records)?;
    let mut items = Vec::new();
    let mut blocked_items = Vec::new();
    for record in &records {
        if record.canonical_bytes.is_empty() {
            blocked_items.push(BlockedItem {
                source_kind: record.source_kind.name().into(),
                source_id: record.source_id.clone(),
                pointer: "/content_json".into(),
                code: "MALFORMED_JSON".into(),
                raw_source: record.raw_source.clone(),
            });
            continue;
        }
        let (kind, raw_source) = compatibility_source(record);
        match map_legacy_with_context(
            LegacySource {
                source_kind: &kind,
                source_id: &record.source_id,
                raw_json: raw_source,
            },
            &context,
        ) {
            Ok(mut mapped) => {
                mapped.raw_source = Some(record.raw_source.clone());
                items.push(PlannedItem {
                    source_kind: record.source_kind.name().into(),
                    source_kind_variant: record.source_kind.clone(),
                    source_id: record.source_id.clone(),
                    source_hash: record.source_hash.clone(),
                    raw_source: record.raw_source.clone(),
                    mapped,
                });
            }
            Err(error) => blocked_items.push(blocked(&error)),
        }
    }
    blocked_items.sort_by(|left, right| {
        (
            &left.source_kind,
            &left.source_id,
            &left.pointer,
            &left.code,
        )
            .cmp(&(
                &right.source_kind,
                &right.source_id,
                &right.pointer,
                &right.code,
            ))
    });
    items.sort_by(|left, right| {
        let rank = |kind: &str| match kind {
            "areas" => 0,
            "projects" => 1,
            "headings" => 2,
            _ => 3,
        };
        (rank(&left.source_kind), &left.source_kind, &left.source_id).cmp(&(
            rank(&right.source_kind),
            &right.source_kind,
            &right.source_id,
        ))
    });
    Ok(ObjectPlan {
        contract_version: CONTRACT_VERSION.into(),
        source_inventory_hash: inventory_hash(&records)?,
        items,
        blocked: blocked_items,
    })
}
