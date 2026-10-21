#![allow(clippy::unwrap_used)]

use crate::type_registry::{delete_object_type, upsert_object_type};
use ark_domain::types::ObjectType;
use rusqlite::Connection;

#[test]
fn load_all_json_is_additive_deterministic_and_hides_deprecated_legacy_types() {
    let connection = Connection::open_in_memory().unwrap();
    crate::open::init_schema(&connection).unwrap();
    for id in ["z-type", "a-type"] {
        upsert_object_type(
            &connection,
            &ObjectType {
                id: id.into(),
                name: id.into(),
                schema_json: "{}".into(),
                ui_schema_json: "{}".into(),
                created_at: "c".into(),
                updated_at: "u".into(),
                system_locked: false,
            },
        )
        .unwrap();
    }
    delete_object_type(&connection, "z-type").unwrap();
    let value = serde_json::to_value(crate::load_all::load_all(&connection).unwrap()).unwrap();
    let keys = value
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    assert!(keys.iter().any(|key| key == "objectTypes"));
    assert!(keys.iter().any(|key| key == "objectTypeSummaries"));
    assert!(keys.iter().any(|key| key == "objectTypeVersions"));
    assert!(keys.iter().any(|key| key == "objectTypeAliases"));
    let legacy_ids = value["objectTypes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value["id"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(!legacy_ids.contains(&"z-type"));
    let registry_ids = value["objectTypeSummaries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value["typeId"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(registry_ids.contains(&"z-type"));
    assert!(registry_ids.windows(2).all(|window| window[0] <= window[1]));
}
