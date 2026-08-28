
#[test]

fn every_compatibility_error_has_stable_code_and_source_coordinates() {
    let cases = [
        (
            map_legacy(&record("unknown_obj", json!({}))).expect_err("unsupported"),
            "UNSUPPORTED_LEGACY_VALUE",
            "/legacy_type_id",
            "unknown_obj",
        ),
        (
            map_legacy(&record("tag_obj", json!({"color": 7}))).expect_err("invalid"),
            "INVALID_FIELD",
            "/props/color",
            "tag_obj",
        ),
        (
            map_legacy(&record(
                "task_obj",
                json!({"scheduledAt":"a", "scheduled_date":"b"}),
            ))
            .expect_err("conflict"),
            "CONFLICTING_FIELDS",
            "/props/scheduledAt",
            "task_obj",
        ),
        (
            map_legacy(&record("note_obj", json!({"api_token":"secret"}))).expect_err("secret"),
            "SECRET_FIELD",
            "/props/api_token",
            "note_obj",
        ),
        (
            {
                let mut source = record("note_obj", json!({}));
                source.content = json!({"type":"doc","content":[{"type":"unknown"}]});
                map_legacy(&source).expect_err("loss risk")
            },
            "DATA_LOSS_RISK",
            "/content/0/type",
            "note_obj",
        ),
    ];
    for (error, code, pointer, source_kind) in cases {
        assert_eq!(error.code(), code);
        assert_eq!(error.source_kind(), source_kind);
        assert_eq!(error.source_id(), format!("{source_kind}-1"));
        assert_eq!(error.pointer(), pointer);
    }
}

#[test]
fn note_and_tag_relations_are_mapped_as_sorted_sets() {
    let note = map_legacy(&record(
        "note_obj",
        json!({
            "related_notes":["n-2","n-1","n-2"], "tag_ids":["t-2","t-1","t-1"]
        }),
    ))
    .unwrap();
    assert_eq!(
        note.links
            .iter()
            .map(|x| (&x.link_type, &x.target_object_id))
            .collect::<Vec<_>>(),
        vec![
            (&"related".to_string(), &"n-1".to_string()),
            (&"related".to_string(), &"n-2".to_string()),
            (&"tag".to_string(), &"t-1".to_string()),
            (&"tag".to_string(), &"t-2".to_string()),
        ]
    );
    let tag = map_legacy(&record(
        "tag_obj",
        json!({"related":["tag-2","tag-1","tag-1"]}),
    ))
    .unwrap();
    assert_eq!(tag.links.len(), 2);
}

#[test]
fn branch_corpus_rejects_types_enums_and_cardinality_with_structured_pointers() {
    let bad_color = record("tag_obj", json!({"color": 7}));
    assert!(
        matches!(map_legacy(&bad_color), Err(CompatibilityError::InvalidField { pointer, .. }) if pointer == "/props/color")
    );
    let bad_project = record("project_obj", json!({"status": "unknown"}));
    assert!(
        matches!(map_legacy(&bad_project), Err(CompatibilityError::InvalidField { pointer, .. }) if pointer == "/props/status")
    );
    let too_many = record("person_obj", json!({"photo_id": ["img-1", "img-2"]}));
    assert!(
        matches!(map_legacy(&too_many), Err(CompatibilityError::InvalidField { pointer, .. }) if pointer == "/props/photo_id")
    );
}

#[test]
fn branch_corpus_preserves_flags_and_rejects_malformed_canonical_values() {
    let flags = map_legacy(&record("task_obj", json!({"is_completed": true}))).unwrap();
    assert_eq!(flags.object.props_json["status"], "done");
    let bad = record("image_obj", json!({"altText": 42}));
    assert!(
        matches!(map_legacy(&bad), Err(CompatibilityError::InvalidField { pointer, .. }) if pointer == "/props/altText")
    );
    let bad_game = record("game_obj", json!({"genres": ["ok", 4]}));
    assert!(
        matches!(map_legacy(&bad_game), Err(CompatibilityError::InvalidField { pointer, .. }) if pointer == "/genres/1"),
        "result: {:?}",
        map_legacy(&bad_game)
    );
}

#[test]
fn non_rich_content_is_not_rich_text_checked_and_existing_links_are_reused() {
    let mut image = record("image_obj", json!({}));
    image.content = json!({"arbitrary": [1, 2, 3]});
    assert!(map_legacy(&image).is_ok());
    let task = record("task_obj", json!({"project_id": "p-1"}));
    let fresh = map_legacy_with_context(
        &task,
        &MappingContext {
            existing_object_ids: [(
                "p-1".into(),
                CanonicalIdentity::new("com.kosmos.project", "1.0.0"),
            )]
            .into_iter()
            .collect(),
            existing_links: vec![ark_core::types::ObjectLink {
                id: "lnk*existing".into(),
                source_object_id: task.id.clone(),
                target_object_id: "p-1".into(),
                link_type: "project".into(),
                created_at: "old".into(),
            }],
        },
    )
    .unwrap();
    assert_eq!(fresh.links[0].id, "lnk*existing");
}

#[test]
fn forbidden_external_and_runtime_fields_never_enter_compatibility_extensions() {
    let mapped = map_legacy(&record(
        "game_obj",
        json!({
            "provider": "steam", "account_id": "acct", "connector_id": "c", "launch_pid": 7,
            "unknown_user_field": "kept"
        }),
    ))
    .unwrap();
    let ext = &mapped.object.props_json["extensions"]["compatibility"];
    assert!(ext.get("provider").is_none());
    assert!(ext.get("accountId").is_none());
    assert!(ext.get("unknownUserField").is_some());
    assert!(mapped.local_state[0].data_json["game"]
        .get("launchPid")
        .is_none());
    assert!(mapped.quarantine[0].fields_json.get("provider").is_some());
}

#[test]
fn rich_text_contract_accepts_valid_marks_and_rejects_unknown_nodes_or_marks() {
    let mut valid = record("note_obj", json!({}));
    valid.content = json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"ok","marks":[{"type":"bold"}]}]}]});
    assert!(map_legacy(&valid).is_ok());
    let mut bad_mark = valid.clone();
    bad_mark.content["content"][0]["content"][0]["marks"][0]["type"] = json!("unknown");
    assert!(
        matches!(map_legacy(&bad_mark), Err(CompatibilityError::DataLossRisk { pointer, .. }) if pointer.starts_with("/content"))
    );
}

#[test]
fn game_existing_object_ids_become_links_only_with_context() {
    let game = record(
        "game_obj",
        json!({"cover_image":"img-known", "background_image":"https://x"}),
    );
    let mut ids = std::collections::BTreeMap::new();
    ids.insert(
        "img-known".into(),
        CanonicalIdentity::new("com.kosmos.image", "1.0.0"),
    );
    let mapped = map_legacy_with_context(
        &game,
        &MappingContext {
            existing_object_ids: ids,
            existing_links: vec![],
        },
    )
    .unwrap();
    assert!(mapped
        .links
        .iter()
        .any(|x| x.link_type == "cover-image" && x.target_object_id == "img-known"));
    assert!(!mapped
        .links
        .iter()
        .any(|x| x.link_type == "background-image"));
    assert_eq!(
        mapped.quarantine[0].fields_json["backgroundImage"],
        "https://x"
    );
}

#[test]
fn cover_links_require_an_existing_image_of_the_canonical_type() {
    let book = record("book_obj", json!({"cover_image": "task-1"}));
    let mut existing = std::collections::BTreeMap::new();
    existing.insert(
        "task-1".into(),
        CanonicalIdentity::new("com.kosmos.task", "1.0.0"),
    );
    let mapped = map_legacy_with_context(
        &book,
        &MappingContext {
            existing_object_ids: existing,
            existing_links: vec![],
        },
    )
    .unwrap();
    assert!(mapped.links.is_empty());
    assert_eq!(mapped.quarantine[0].fields_json["coverImage"], "task-1");

    let mut existing = std::collections::BTreeMap::new();
    existing.insert(
        "image-1".into(),
        CanonicalIdentity::new("com.kosmos.image", "1.0.0"),
    );
    let image_book = record("book_obj", json!({"cover_image": "image-1"}));
    let mapped = map_legacy_with_context(
        &image_book,
        &MappingContext {
            existing_object_ids: existing,
            existing_links: vec![],
        },
    )
    .unwrap();
    assert_eq!(mapped.links.len(), 1);
    assert_eq!(mapped.links[0].link_type, "cover-image");
    assert!(mapped.quarantine.is_empty());
}

#[test]
fn canonical_relation_aliases_reach_links_and_conflicts_are_source_pointed() {
    let task = map_legacy(&record("task_obj", json!({"projectId":"p-1"}))).unwrap();
    assert_eq!(task.links[0].target_object_id, "p-1");
    let conflict = record("task_obj", json!({"projectId":"p-1", "project_id":"p-2"}));
    assert!(
        matches!(map_legacy(&conflict), Err(CompatibilityError::ConflictingFields { pointer, .. }) if pointer == "/props/projectId")
    );
    let time = map_legacy(&record(
        "time_entry_obj",
        json!({"startedAt":"2026-01-01T00:00:00Z", "taskId":"t-1"}),
    ))
    .unwrap();
    assert_eq!(time.links[0].target_object_id, "t-1");
}

#[test]
fn game_rating_is_bounded_and_non_numeric_values_are_rejected() {
    for value in [json!(0), json!(10), json!(5.5)] {
        assert!(map_legacy(&record("game_obj", json!({"userRating":value}))).is_ok());
    }
    for value in [json!(-1), json!(11), json!("5"), json!(true)] {
        assert!(
            matches!(map_legacy(&record("game_obj", json!({"userRating":value}))), Err(CompatibilityError::InvalidField { pointer, .. }) if pointer == "/props/userRating")
        );
    }
}