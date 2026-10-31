#![allow(clippy::unwrap_used)]

use crate::type_registry::{
    delete_object_type, get_type, list_type_summaries, list_type_versions, upsert_object_type,
};
use ark_domain::canonical_type::definitions::canonical_type_registrations;
use ark_domain::types::ObjectType;
use rusqlite::Connection;

fn setup() -> Connection {
    let connection = Connection::open_in_memory().unwrap();
    crate::open::init_schema(&connection).unwrap();
    connection
}

#[test]
fn fresh_init_registers_core_canonical_authority_without_aliases() {
    let connection = setup();
    let registrations = canonical_type_registrations().expect("canonical definitions");
    let summaries = list_type_summaries(&connection).unwrap();
    let unique_type_ids = registrations
        .iter()
        .map(|registration| registration.type_id.as_str())
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(summaries.len(), unique_type_ids.len());

    for registration in registrations {
        assert!(registration.type_id.starts_with("core."));
        assert!(registration.version == "0.1.0"
            || registration.type_id == "core.task" && registration.version == "0.2.0");
        assert!(registration.aliases.is_empty());
        let versions = list_type_versions(&connection, &registration.type_id).unwrap();
        assert!(versions.iter().any(|version| {
            version.version == registration.version
                && version.schema_hash == registration.schema_hash
                && !version.schema_json.contains("cover_image")
                && !version.schema_json.contains("rawg_id")
        }));
    }
}

#[test]
fn inbound_legacy_object_type_entity_cannot_mutate_canonical_row() {
    let connection = setup();
    let synced = ObjectType {
        id: "core.task".into(),
        name: "Peer Task".into(),
        schema_json: "{}".into(),
        ui_schema_json: "{}".into(),
        created_at: "now".into(),
        updated_at: "now".into(),
        system_locked: false,
    };
    upsert_object_type(&connection, &synced).unwrap();
    delete_object_type(&connection, "core.task").unwrap();

    let canonical = get_type(&connection, "core.task", None)
        .unwrap()
        .expect("canonical type remains registered");
    assert_eq!(canonical.summary.current_version, "0.2.0");
    assert_eq!(canonical.summary.status, "active");
    assert!(list_type_versions(&connection, "core.task")
        .unwrap()
        .iter()
        .all(|version| !version.version.starts_with("0.0.0+legacy.")));
}
