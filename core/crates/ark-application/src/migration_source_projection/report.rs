use super::inventory::{compact, generic_inventory, hash_bytes};
use super::native_sources::native_inventory;
use super::*;
use crate::canonical_compatibility::{map_legacy_with_context, LegacySource, MappingContext};
use crate::source_snapshot::SourceSnapshot;
use serde_json::{json, Value};

pub fn preflight_from_snapshot(
    snapshot: &SourceSnapshot,
    ctx: &MappingContext,
) -> Result<MigrationPreflightReport, String> {
    let mut blocked = Vec::new();
    let mut records = Vec::new();
    generic_inventory(snapshot, &mut records, &mut blocked)?;
    native_inventory(snapshot, &mut records)?;
    records.sort_by(|a, b| {
        (a.source_kind.name(), &a.source_id).cmp(&(b.source_kind.name(), &b.source_id))
    });
    let inventory_bytes = compact(&Value::Array(
        records
            .iter()
            .map(|r| {
                json!(
                    {"sourceKind":r.source_kind.name(),
                    "sourceId":r.source_id,
                    "sourceHash":r.source_hash})
            })
            .collect(),
    ))?;
    let inventory_hash = hash_bytes(&inventory_bytes);
    let mut errors = blocked.clone();
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
            ctx,
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
