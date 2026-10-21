use super::mapping_context;
use crate::source_snapshot::{ObjectIdentityRow, SourceSnapshot};

#[test]
fn mapping_context_resolves_aliases_and_keeps_existing_links() {
    let registration = ark_domain::canonical_type::definitions::canonical_type_registrations()
        .expect("canonical registrations load")
        .into_iter()
        .find(|registration| !registration.aliases.is_empty())
        .expect("at least one canonical type has an alias");
    let alias = registration.aliases[0].alias.clone();
    let link = ark_domain::types::ObjectLink {
        id: "link-1".into(),
        source_object_id: "source".into(),
        target_object_id: "target".into(),
        link_type: "related".into(),
        created_at: "2026-01-01T00:00:00.000Z".into(),
    };
    let snapshot = SourceSnapshot {
        object_identities: vec![ObjectIdentityRow {
            id: "existing".into(),
            type_id: alias,
            type_version: "legacy-version".into(),
        }],
        object_links: vec![link.clone()],
        ..SourceSnapshot::default()
    };

    let context = mapping_context(&snapshot).expect("mapping context builds");

    assert_eq!(
        context.existing_object_ids.get("existing"),
        Some(&crate::canonical_compatibility::CanonicalIdentity::new(
            registration.type_id,
            registration.version,
        ))
    );
    assert_eq!(context.existing_links, [link]);
}
