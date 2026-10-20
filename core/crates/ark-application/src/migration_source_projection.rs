mod inventory;
mod native_sources;
mod planning;
mod report;

use crate::canonical_compatibility::{CanonicalIdentity, MappingContext};
use crate::source_snapshot::SourceSnapshot;
use std::collections::BTreeMap;

pub use inventory::{
    BlockedItem, MigrationPreflightReport, PreflightType, SourceKind, SourceRecord,
};
pub use native_sources::inventory_snapshot;
pub use planning::{
    plan_objects_from_snapshot, ObjectPlan, ObjectPlanError, PlannedItem, CONTRACT_VERSION,
};
pub use report::preflight_from_snapshot;

pub(super) fn canonical_aliases() -> Result<BTreeMap<String, CanonicalIdentity>, String> {
    Ok(
        ark_domain::canonical_type::definitions::canonical_type_registrations()?
            .into_iter()
            .flat_map(|registration| {
                registration.aliases.into_iter().map(move |alias| {
                    (
                        alias.alias,
                        CanonicalIdentity::new(
                            registration.type_id.clone(),
                            registration.version.clone(),
                        ),
                    )
                })
            })
            .collect(),
    )
}

pub fn mapping_context(snapshot: &SourceSnapshot) -> Result<MappingContext, String> {
    let aliases = canonical_aliases()?;
    let existing_object_ids = snapshot
        .object_identities
        .iter()
        .map(|identity| {
            (
                identity.id.clone(),
                aliases.get(&identity.type_id).cloned().unwrap_or_else(|| {
                    CanonicalIdentity::new(identity.type_id.clone(), identity.type_version.clone())
                }),
            )
        })
        .collect();
    Ok(MappingContext {
        existing_object_ids,
        existing_links: snapshot.object_links.clone(),
    })
}

#[cfg(test)]
#[path = "migration_source_projection_tests.rs"]
mod tests;
