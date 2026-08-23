use ark_core::canonical_types::definitions::canonical_type_registrations;
use ark_core::canonical_types::validation::{validate_canonical, CanonicalValidationCode};
use serde_json::{json, Value};

fn registration(type_id: &str) -> ark_core::type_registry::TypeRegistration {
    canonical_type_registrations()
        .unwrap()
        .into_iter()
        .find(|registration| registration.type_id == type_id)
        .unwrap()
}

fn valid_task_props() -> Value {
    json!({
        "status": "todo", "priority": "medium", "scheduledAt": null,
        "dueAt": null, "reminderAt": null, "completedAt": null,
        "canceledAt": null, "recurrence": null, "checklist": [],
        "extensions": { "vendor": { "opaque": true } }
    })
}

fn valid_doc() -> Value {
    json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"hello","marks":[{"type":"bold"}]}]}]})
}

fn error(
    type_id: &str,
    props: Value,
    content: Value,
) -> ark_core::canonical_types::validation::CanonicalValidationError {
    validate_canonical(&registration(type_id), &props, &content).unwrap_err()
}

fn with_contract(
    mut registration: ark_core::type_registry::TypeRegistration,
    contract: Value,
) -> ark_core::type_registry::TypeRegistration {
    registration.content_contract_json = serde_json::to_string(&contract).unwrap();
    registration
}

fn content_contract_error(
    contract: Value,
) -> ark_core::canonical_types::validation::CanonicalValidationError {
    validate_canonical(
        &with_contract(registration("com.kosmos.note"), contract),
        &json!({"description":null,"extensions":{}}),
        &valid_doc(),
    )
    .unwrap_err()
}

#[test]
fn malformed_content_contract_is_an_invariant_at_stable_pointer() {
    let mut malformed_registration = registration("com.kosmos.note");
    malformed_registration.content_contract_json = "{".into();
    let failure = validate_canonical(
        &malformed_registration,
        &json!({"description":null,"extensions":{}}),
        &valid_doc(),
    )
    .unwrap_err();
    assert_eq!(failure.code, CanonicalValidationCode::InvariantViolation);
    assert_eq!(failure.pointer, "");
    assert_eq!(failure.keyword.as_deref(), Some("contentContract"));

    let mut contract: Value =
        serde_json::from_str(&registration("com.kosmos.note").content_contract_json).unwrap();
    contract["mediaType"] = json!("application/vnd.kosmos.unknown");
    let failure = content_contract_error(contract);
    assert_eq!(failure.code, CanonicalValidationCode::InvariantViolation);
    assert_eq!(failure.pointer, "/mediaType");
}

#[test]
fn content_contract_shape_and_parity_fail_closed_before_payload_validation() {
    let base: Value =
        serde_json::from_str(&registration("com.kosmos.note").content_contract_json).unwrap();
    for (field, value) in [
        ("mediaType", json!(4)),
        ("version", json!("1")),
        ("rootType", json!("paragraph")),
        ("allowedNodes", json!("doc")),
        ("allowedMarks", json!(["bold", 4])),
        ("attributes", json!([])),
    ] {
        let mut contract = base.clone();
        contract[field] = value;
        let failure = content_contract_error(contract);
        assert_eq!(
            failure.code,
            CanonicalValidationCode::InvariantViolation,
            "{field}"
        );
        assert_eq!(failure.pointer, format!("/{field}"), "{field}");
    }
    let mut contract = base.clone();
    contract["allowedNodes"]
        .as_array_mut()
        .unwrap()
        .push(json!("futureNode"));
    let failure = content_contract_error(contract);
    assert_eq!(failure.code, CanonicalValidationCode::InvariantViolation);
    assert_eq!(failure.pointer, "/allowedNodes");

    for field in [
        "mediaType",
        "version",
        "rootType",
        "allowedNodes",
        "allowedMarks",
        "attributes",
    ] {
        let mut contract = base.clone();
        contract.as_object_mut().unwrap().remove(field);
        let failure = content_contract_error(contract);
        assert_eq!(
            failure.code,
            CanonicalValidationCode::InvariantViolation,
            "{field}"
        );
        assert_eq!(failure.pointer, format!("/{field}"), "{field}");
    }
    let mut contract = base;
    contract["extra"] = json!(true);
    let failure = content_contract_error(contract);
    assert_eq!(failure.code, CanonicalValidationCode::InvariantViolation);
    assert_eq!(failure.pointer, "/extra");
}

#[test]
fn schema_keyword_values_are_definition_invariants_recursively() {
    let cases = [
        ("$schema", json!("wrong"), "/$schema"),
        ("type", json!(123), "/properties/status/type"),
        ("required", json!("description"), "/required"),
        ("required", json!([4]), "/required"),
        ("properties", json!([]), "/properties"),
        (
            "additionalProperties",
            json!("false"),
            "/additionalProperties",
        ),
        ("enum", json!("todo"), "/properties/status/enum"),
        (
            "minimum",
            json!("1"),
            "/properties/recurrence/properties/interval/minimum",
        ),
        ("maximum", json!([]), "/properties/status/maximum"),
        (
            "minLength",
            json!(-1),
            "/properties/checklist/items/minLength",
        ),
        ("items", json!([]), "/properties/checklist/items/items"),
        (
            "uniqueItems",
            json!("true"),
            "/properties/checklist/items/uniqueItems",
        ),
        ("format", json!(false), "/properties/scheduledAt/format"),
    ];
    for (keyword, value, expected_pointer) in cases {
        let mut registration = registration("com.kosmos.task");
        let mut schema: Value = serde_json::from_str(&registration.schema_json).unwrap();
        let target = if expected_pointer.starts_with("/properties/checklist") {
            &mut schema["properties"]["checklist"]["items"]
        } else if expected_pointer.contains("recurrence/properties/interval") {
            &mut schema["properties"]["recurrence"]["properties"]["interval"]
        } else if expected_pointer.contains("userRating") {
            &mut schema["properties"]["userRating"]
        } else if expected_pointer.contains("status") {
            &mut schema["properties"]["status"]
        } else if expected_pointer.contains("scheduledAt") {
            &mut schema["properties"]["scheduledAt"]
        } else {
            &mut schema
        };
        target[keyword] = value;
        registration.schema_json = serde_json::to_string(&schema).unwrap();
        let failure =
            validate_canonical(&registration, &valid_task_props(), &valid_doc()).unwrap_err();
        assert_eq!(
            failure.code,
            CanonicalValidationCode::InvariantViolation,
            "{keyword}"
        );
        assert_eq!(failure.pointer, expected_pointer, "{keyword}");
    }
}

#[test]
fn rich_text_uses_contract_for_nodes_marks_and_attributes() {
    let base = json!({"description":null,"extensions":{}});
    let cases = [
        (json!({"type":"doc","extra":true}), "/extra", "nodeShape"),
        (
            json!({"type":"doc","content":[{"type":"paragraph","marks":[{"type":"bold","extra":true}]}]}),
            "/content/0/marks/0/extra",
            "markShape",
        ),
        (
            json!({"type":"doc","content":[{"type":"heading","attrs":{"unknown":1}}]}),
            "/content/0/attrs/unknown",
            "attributes",
        ),
        (
            json!({"type":"doc","content":[{"type":"paragraph","marks":[{"type":"link","attrs":{}}]}]}),
            "/content/0/marks/0/attrs/href",
            "required",
        ),
        (
            json!({"type":"doc","content":[{"type":"paragraph","marks":[{"type":"link","attrs":{"href":null}}]}]}),
            "/content/0/marks/0/attrs/href",
            "type",
        ),
    ];
    for (doc, pointer, keyword) in cases {
        let failure = error("com.kosmos.note", base.clone(), doc);
        assert_eq!(failure.code, CanonicalValidationCode::InvalidField);
        assert_eq!(failure.pointer, pointer);
        assert_eq!(failure.keyword.as_deref(), Some(keyword));
    }
}

#[test]
fn validates_frozen_task_props_and_open_extensions() {
    assert!(validate_canonical(
        &registration("com.kosmos.task"),
        &valid_task_props(),
        &valid_doc()
    )
    .is_ok());
    let mut props = valid_task_props();
    props["unexpected"] = json!(true);
    let failure = error("com.kosmos.task", props, valid_doc());
    assert_eq!(failure.code, CanonicalValidationCode::InvalidField);
    assert_eq!(failure.pointer, "/unexpected");
}

#[test]
fn reports_required_type_enum_and_rfc6901_pointers() {
    let mut props = valid_task_props();
    props.as_object_mut().unwrap().remove("priority");
    assert_eq!(
        error("com.kosmos.task", props, valid_doc()).pointer,
        "/priority"
    );
    let mut props = valid_task_props();
    props["priority"] = json!(4);
    assert_eq!(
        error("com.kosmos.task", props, valid_doc()).pointer,
        "/priority"
    );
    let mut props = valid_task_props();
    props["status"] = json!("unknown");
    assert_eq!(
        error("com.kosmos.task", props, valid_doc()).pointer,
        "/status"
    );
    let mut props = valid_task_props();
    props["extensions"] = json!({"a/b": {"~key": false}});
    assert!(validate_canonical(&registration("com.kosmos.task"), &props, &valid_doc()).is_ok());
}

#[test]
fn enforces_task_recurrence_and_checklist_bounds() {
    let mut props = valid_task_props();
    props["recurrence"] = json!({"frequency":"weekly","interval":0,"recurrenceType":"fixed","daysOfWeek":[1,1],"endDate":null});
    assert_eq!(
        error("com.kosmos.task", props, valid_doc()).pointer,
        "/recurrence/interval"
    );
    let mut props = valid_task_props();
    props["recurrence"] = json!({"frequency":"weekly","interval":1,"recurrenceType":"fixed","daysOfWeek":[1,1],"endDate":null});
    assert_eq!(
        error("com.kosmos.task", props, valid_doc()).pointer,
        "/recurrence/daysOfWeek"
    );
    let mut props = valid_task_props();
    props["checklist"] = json!([{"id":"","title":"x","isCompleted":false}]);
    assert_eq!(
        error("com.kosmos.task", props, valid_doc()).pointer,
        "/checklist/0/id"
    );
}

#[test]
fn enforces_numeric_bounds_and_integer_semantics() {
    let mut game = json!({"playStatus":null,"userRating":11,"genres":[],"platforms":[],"released":null,"description":null,"extensions":{}});
    assert_eq!(
        error("com.kosmos.game", game.clone(), json!({"shape":"object"})).pointer,
        "/userRating"
    );
    game["userRating"] = json!(4.5);
    assert!(validate_canonical(
        &registration("com.kosmos.game"),
        &game,
        &json!({"shape":"object"})
    )
    .is_ok());
    let mut image = json!({"fileName":null,"mimeType":null,"sizeBytes":-1,"width":null,"height":null,"resolution":null,"altText":"","extensions":{}});
    assert_eq!(
        error("com.kosmos.image", image.clone(), json!({"shape":"object"})).pointer,
        "/sizeBytes"
    );
    image["sizeBytes"] = json!(1.2);
    assert_eq!(
        error("com.kosmos.image", image, json!({"shape":"object"})).pointer,
        "/sizeBytes"
    );
}

#[test]
fn format_is_annotation_but_rich_text_is_strict() {
    let props = json!({"startedAt":"not-a-date","endedAt":null,"billable":false,"source":"manual","taskTitle":null,"extensions":{}});
    assert!(
        validate_canonical(&registration("com.kosmos.time-entry"), &props, &valid_doc()).is_ok()
    );
    let mut doc = valid_doc();
    doc["content"][0]["content"][0]["marks"][0] = json!({"type":"unknown"});
    let failure = error(
        "com.kosmos.note",
        json!({"description":null,"extensions":{}}),
        doc,
    );
    assert_eq!(failure.pointer, "/content/0/content/0/marks/0/type");
    let invalid_node = json!({"type":"doc","content":[{"type":"unknown"}]});
    assert_eq!(
        error(
            "com.kosmos.note",
            json!({"description":null,"extensions":{}}),
            invalid_node
        )
        .pointer,
        "/content/0/type"
    );
}

#[test]
fn validates_rich_text_attribute_types_and_bounds() {
    let base = json!({"description":null,"extensions":{}});
    for (node, pointer) in [
        (
            json!({"type":"heading","attrs":{"level":7}}),
            "/content/0/attrs/level",
        ),
        (
            json!({"type":"orderedList","attrs":{"start":0}}),
            "/content/0/attrs/start",
        ),
        (
            json!({"type":"image","attrs":{"src":4}}),
            "/content/0/attrs/src",
        ),
        (
            json!({"type":"taskItem","attrs":{"checked":"yes"}}),
            "/content/0/attrs/checked",
        ),
    ] {
        let doc = json!({"type":"doc","content":[node]});
        assert_eq!(error("com.kosmos.note", base.clone(), doc).pointer, pointer);
    }
    let valid = json!({"type":"doc","content":[{"type":"heading","attrs":{"level":6},"content":[{"type":"text","text":"x"}]}]});
    assert!(validate_canonical(&registration("com.kosmos.note"), &base, &valid).is_ok());
}

#[test]
fn fails_closed_on_unsupported_schema_keyword() {
    let mut registration = registration("com.kosmos.note");
    let mut schema: Value = serde_json::from_str(&registration.schema_json).unwrap();
    schema["unevaluatedProperties"] = json!(false);
    registration.schema_json = serde_json::to_string(&schema).unwrap();
    let failure = validate_canonical(
        &registration,
        &json!({"description":null,"extensions":{}}),
        &valid_doc(),
    )
    .unwrap_err();
    assert_eq!(failure.code, CanonicalValidationCode::InvariantViolation);
    assert_eq!(failure.pointer, "/unevaluatedProperties");
}
