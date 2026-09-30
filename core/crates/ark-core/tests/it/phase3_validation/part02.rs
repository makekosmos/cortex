
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

