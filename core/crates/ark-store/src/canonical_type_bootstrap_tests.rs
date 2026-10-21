#![allow(clippy::unwrap_used)]

use crate::type_registry::{
    delete_object_type, get_type, list_type_summaries, list_type_versions, resolve_alias,
    upsert_object_type,
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
fn fresh_init_registers_canonical_authority_and_routes_legacy_aliases() {
    let connection = setup();
    let registrations = canonical_type_registrations().expect("canonical definitions");
    let summaries = list_type_summaries(&connection).unwrap();
    let unique_type_ids = registrations
        .iter()
        .map(|registration| registration.type_id.as_str())
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(summaries.len(), unique_type_ids.len());

    for registration in registrations {
        let versions = list_type_versions(&connection, &registration.type_id).unwrap();
        assert!(versions.iter().any(|version| {
            version.version == registration.version
                && version.schema_hash == registration.schema_hash
                && !version.schema_json.contains("cover_image")
                && !version.schema_json.contains("rawg_id")
        }));
        for alias in registration.aliases {
            assert_eq!(
                resolve_alias(&connection, &alias.alias).unwrap().as_deref(),
                Some(registration.type_id.as_str())
            );
        }
    }

    let legacy = ObjectType {
        id: "note_obj".into(),
        name: "legacy authority".into(),
        schema_json: "{}".into(),
        ui_schema_json: "{}".into(),
        created_at: "now".into(),
        updated_at: "now".into(),
        system_locked: false,
    };
    let error = upsert_object_type(&connection, &legacy).unwrap_err();
    assert!(error.contains("alias collides with canonical type"));
    assert_eq!(
        get_type(&connection, "note_obj", None)
            .unwrap()
            .unwrap()
            .summary
            .type_id,
        "com.kosmos.note"
    );
}

#[test]
fn inbound_legacy_object_type_entity_cannot_mutate_canonical_row() {
    let connection = setup();
    let synced = ObjectType {
        id: "com.kosmos.task".into(),
        name: "Peer Task".into(),
        schema_json: "{}".into(),
        ui_schema_json: "{}".into(),
        created_at: "now".into(),
        updated_at: "now".into(),
        system_locked: false,
    };
    upsert_object_type(&connection, &synced).unwrap();
    delete_object_type(&connection, "com.kosmos.task").unwrap();

    let canonical = get_type(&connection, "com.kosmos.task", None)
        .unwrap()
        .expect("canonical type remains registered");
    assert_eq!(canonical.summary.current_version, "1.1.0");
    assert_eq!(canonical.summary.status, "active");
    assert!(list_type_versions(&connection, "com.kosmos.task")
        .unwrap()
        .iter()
        .all(|version| !version.version.starts_with("0.0.0+legacy.")));
}
