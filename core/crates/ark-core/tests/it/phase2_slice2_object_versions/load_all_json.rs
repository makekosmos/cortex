#[test]

fn load_all_json_is_additive_deterministic_and_hides_deprecated_legacy_types() {
    let conn = setup();
    for id in ["z-type", "a-type"] {
        upsert_object_type(
            &conn,
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
    ark_core::db::delete_object_type(&conn, "z-type").unwrap();
    let value = serde_json::to_value(load_all(&conn).unwrap()).unwrap();
    let keys = value
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    assert!(keys.iter().any(|k| k == "objectTypes"));
    assert!(keys.iter().any(|k| k == "objectTypeSummaries"));
    assert!(keys.iter().any(|k| k == "objectTypeVersions"));
    assert!(keys.iter().any(|k| k == "objectTypeAliases"));
    let legacy_ids = value["objectTypes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["id"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(!legacy_ids.contains(&"z-type"));
    let registry_ids = value["objectTypeSummaries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["typeId"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(registry_ids.contains(&"z-type"));
    assert!(registry_ids.windows(2).all(|w| w[0] <= w[1]));
}
