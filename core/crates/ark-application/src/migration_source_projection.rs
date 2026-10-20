mod inventory;
mod native_sources;
mod report;

use crate::canonical_compatibility::{CanonicalIdentity, MappingContext};
use crate::source_snapshot::SourceSnapshot;
use std::collections::BTreeMap;

pub use inventory::{
    BlockedItem, MigrationPreflightReport, PreflightType, SourceKind, SourceRecord,
};
pub use native_sources::inventory_snapshot;
pub use report::preflight_from_snapshot;

pub fn mapping_context(snapshot: &SourceSnapshot) -> Result<MappingContext, String> {
    let aliases: BTreeMap<String, CanonicalIdentity> =
        ark_domain::canonical_type::definitions::canonical_type_registrations()?
            .iter()
            .flat_map(|registration| {
                registration.aliases.iter().map(move |alias| {
                    (
                        alias.alias.clone(),
                        CanonicalIdentity::new(
                            registration.type_id.clone(),
                            registration.version.clone(),
                        ),
                    )
                })
            })
            .collect();
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
