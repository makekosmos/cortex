use super::definitions::canonical_type_registrations;
use ark_application::canonical_compatibility::{CanonicalIdentity, MappingContext};
use ark_application::source_snapshot::SourceSnapshot;
use rusqlite::Connection;
use std::collections::BTreeMap;

pub use ark_application::migration_source_projection::{
    BlockedItem, MigrationPreflightReport, PreflightType, SourceKind, SourceRecord,
};

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
    let ids = ark_store::objects::list_object_type_identities(conn)?
        .into_iter()
        .map(|(id, identity)| {
            (
                id,
                aliases.get(&identity.type_id).cloned().unwrap_or_else(|| {
                    CanonicalIdentity::new(identity.type_id, identity.type_version)
                }),
            )
        })
        .collect();
    Ok(MappingContext {
        existing_object_ids: ids,
        existing_links: ark_store::objects::list_object_links(conn)?,
    })
}

pub fn inventory_sources(conn: &Connection) -> Result<Vec<SourceRecord>, String> {
    let snapshot = ark_store::source_snapshot::read_source_snapshot(conn)?;
    ark_application::migration_source_projection::inventory_snapshot(&snapshot)
}

pub fn preflight_phase3(conn: &Connection) -> Result<MigrationPreflightReport, String> {
    let snapshot: SourceSnapshot = ark_store::source_snapshot::read_source_snapshot(conn)?;
    let context = context(conn)?;
    ark_application::migration_source_projection::preflight_from_snapshot(&snapshot, &context)
}
