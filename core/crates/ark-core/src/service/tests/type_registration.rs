use super::*;
use serde_json::Value;

const SCHEMA_V1: &str = concat!(
    r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","#,
    r#""type":"object","properties":{"title":{"type":"string"},"#,
    r#""priority":{"type":"integer"}},"required":["title"]}"#
);

const SCHEMA_V1_1: &str = concat!(
    r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","#,
    r#""type":"object","properties":{"title":{"type":"string"},"#,
    r#""priority":{"type":"integer"},"note":{"type":"string"}},"#,
    r#""required":["title"]}"#
);

fn schema(raw: &str) -> Value {
    serde_json::from_str(raw).expect("test schema is valid JSON")
}

fn register(type_id: &str, version: &str, raw_schema: &str, app_id: Option<&str>) -> Request {
    Request::TypesRegister {
        type_id: type_id.to_owned(),
        version: version.to_owned(),
        schema: schema(raw_schema),
        name: None,
        app_id: app_id.map(str::to_owned),
    }
}

fn summary<'a>(list: &'a Value, type_id: &str) -> &'a Value {
    list.as_array()
        .expect("types.list returns an array")
        .iter()
        .find(|item| item["typeId"] == type_id)
        .unwrap_or_else(|| panic!("type {type_id} missing from types.list: {list}"))
}

#[test]
fn wire_contract_uses_the_dotted_operation_name_and_camel_case_fields() {
    let request: Request = serde_json::from_value(serde_json::json!({
        "operation": "types.register",
        "typeId": "com.example.note",
        "version": "1.0.0",
        "appId": "com.example",
        "name": "Note",
        "schema": {
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object"
        }
    }))
    .expect("types.register wire payload is accepted");
    let Request::TypesRegister {
        type_id,
        app_id,
        schema,
        ..
    } = request
    else {
        panic!("expected TypesRegister");
    };
    assert_eq!(type_id, "com.example.note");
    assert_eq!(app_id.as_deref(), Some("com.example"));
    assert_eq!(schema["type"], "object");
}

#[tokio::test]
async fn registers_a_com_type_and_exposes_it_in_types_list() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    initialize_test_database(&state, fixture.db_path().to_string_lossy().into_owned())
        .await
        .unwrap();

    let response = handle_request(
        &state,
        register("com.example.notes.note", "1.0.0", SCHEMA_V1, None),
    )
    .await
    .unwrap();
    assert_eq!(response["typeId"], "com.example.notes.note");
    assert_eq!(response["version"], "1.0.0");

    let list = handle_request(&state, Request::TypesList).await.unwrap();
    let registered = summary(&list, "com.example.notes.note");
    assert_eq!(registered["currentVersion"], "1.0.0");
    assert_eq!(registered["ownerKind"], "app");
    assert_eq!(registered["ownerId"], "com.example.notes");
    fixture.shutdown().await;
}

#[tokio::test]
async fn rejects_types_outside_the_callers_namespace_and_system_types() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    initialize_test_database(&state, fixture.db_path().to_string_lossy().into_owned())
        .await
        .unwrap();

    let foreign = handle_request(
        &state,
        register(
            "com.other.notes.note",
            "1.0.0",
            SCHEMA_V1,
            Some("com.example.notes"),
        ),
    )
    .await
    .unwrap_err();
    assert!(foreign.contains("outside namespace"), "{foreign}");

    let not_com = handle_request(
        &state,
        register("org.example.notes.note", "1.0.0", SCHEMA_V1, None),
    )
    .await
    .unwrap_err();
    assert!(not_com.contains("com.<app>.*"), "{not_com}");

    let system = handle_request(&state, register("core.note", "9.9.9", SCHEMA_V1, None))
        .await
        .unwrap_err();
    assert!(system.contains("system type"), "{system}");

    let list = handle_request(&state, Request::TypesList).await.unwrap();
    assert!(
        !list
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["typeId"] == "com.other.notes.note"),
        "rejected registration must not be persisted"
    );
    fixture.shutdown().await;
}

#[tokio::test]
async fn rejects_incompatible_schema_changes_with_a_clear_reason() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    initialize_test_database(&state, fixture.db_path().to_string_lossy().into_owned())
        .await
        .unwrap();
    handle_request(
        &state,
        register("com.example.notes.note", "1.0.0", SCHEMA_V1, None),
    )
    .await
    .unwrap();

    let changed_type = handle_request(
        &state,
        register(
            "com.example.notes.note",
            "1.1.0",
            r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","properties":{"title":{"type":"string"},"priority":{"type":"string"}},"required":["title"]}"#,
            None,
        ),
    )
    .await
    .unwrap_err();
    assert!(
        changed_type.contains("not backward compatible")
            && changed_type.contains("priority")
            && changed_type.contains("changed type"),
        "{changed_type}"
    );

    let removed = handle_request(
        &state,
        register(
            "com.example.notes.note",
            "1.1.0",
            r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","properties":{"title":{"type":"string"}},"required":["title"]}"#,
            None,
        ),
    )
    .await
    .unwrap_err();
    assert!(
        removed.contains("was removed") && removed.contains("priority"),
        "{removed}"
    );

    let list = handle_request(&state, Request::TypesList).await.unwrap();
    assert_eq!(
        summary(&list, "com.example.notes.note")["currentVersion"],
        "1.0.0",
        "rejected versions must not move the current version"
    );
    fixture.shutdown().await;
}

#[tokio::test]
async fn registered_type_and_versions_survive_a_service_restart() {
    let fixture = service_fixture();
    let state = fixture.state.clone();
    let db_path = fixture.db_path().to_string_lossy().into_owned();
    initialize_test_database(&state, db_path.clone())
        .await
        .unwrap();
    handle_request(
        &state,
        register("com.example.notes.note", "1.0.0", SCHEMA_V1, None),
    )
    .await
    .unwrap();
    handle_request(
        &state,
        register("com.example.notes.note", "1.1.0", SCHEMA_V1_1, None),
    )
    .await
    .unwrap();

    shutdown_test_state(&state).await;
    initialize_test_database(&state, db_path).await.unwrap();

    let list = handle_request(&state, Request::TypesList).await.unwrap();
    assert_eq!(
        summary(&list, "com.example.notes.note")["currentVersion"],
        "1.1.0"
    );
    let versions = handle_request(
        &state,
        Request::TypesListVersions {
            type_id: "com.example.notes.note".into(),
        },
    )
    .await
    .unwrap();
    let versions = versions.as_array().unwrap();
    assert_eq!(versions.len(), 2, "{versions:?}");
    let definition = handle_request(
        &state,
        Request::TypesGet {
            type_id: "com.example.notes.note".into(),
            version: Some("1.1.0".into()),
        },
    )
    .await
    .unwrap();
    assert!(
        definition["definition"]["schemaJson"]
            .as_str()
            .unwrap_or_default()
            .contains("note"),
        "{definition}"
    );
    fixture.shutdown().await;
}
