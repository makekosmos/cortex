
#[test]

fn book_cover_links_require_known_image_context_and_reuse_existing_id() {
    let book = record("book_obj", json!({"coverImage":"img-1"}));
    let mut ids = std::collections::BTreeMap::new();
    ids.insert(
        "img-1".into(),
        CanonicalIdentity::new("com.kosmos.image", "1.0.0"),
    );
    let existing = ark_core::types::ObjectLink {
        id: "lnk*old".into(),
        source_object_id: book.id.clone(),
        target_object_id: "img-1".into(),
        link_type: "cover-image".into(),
        created_at: "old".into(),
    };
    let mapped = map_legacy_with_context(
        &book,
        &MappingContext {
            existing_object_ids: ids,
            existing_links: vec![existing],
        },
    )
    .unwrap();
    assert_eq!(
        mapped
            .links
            .iter()
            .find(|l| l.link_type == "cover-image")
            .unwrap()
            .id,
        "lnk*old"
    );
    assert!(mapped
        .quarantine
        .iter()
        .all(|q| q.fields_json.get("coverImage").is_none()));
}

#[test]
fn relation_alias_sets_ignore_order_and_duplicates_but_detect_real_set_conflicts() {
    let equal = record(
        "task_obj",
        json!({"tagIds":["t-2","t-1","t-2"], "tag_ids":["t-1","t-2"]}),
    );
    assert!(map_legacy(&equal).is_ok());
    let conflict = record(
        "task_obj",
        json!({"tagIds":["t-1"], "tag_ids":["t-1","t-2"]}),
    );
    assert!(
        matches!(map_legacy(&conflict), Err(CompatibilityError::ConflictingFields { pointer, .. }) if pointer == "/props/tagIds")
    );
}

#[test]
fn context_rejects_missing_relation_targets_and_preserves_raw_bytes_on_errors() {
    let task = record("task_obj", json!({"project_id":"p-1"}));
    let raw = serde_json::to_vec(&task).unwrap();
    let error = map_legacy_with_context(
        &task,
        &MappingContext {
            existing_object_ids: std::collections::BTreeMap::new(),
            existing_links: vec![],
        },
    )
    .expect_err("ordinary relation target must exist in context");
    assert!(matches!(
        error,
        CompatibilityError::InvalidField { ref pointer, .. } if pointer == "/props/project_id"
    ));
    assert_eq!(error.source_kind(), "task_obj");
    assert_eq!(error.source_id(), "task_obj-1");
    assert_eq!(error.raw_source(), raw.as_slice());

    let mut known = std::collections::BTreeMap::new();
    known.insert(
        "p-1".into(),
        CanonicalIdentity::new("com.kosmos.project", "1.0.0"),
    );
    for props in [json!({"project_id":"p-1"}), json!({"projectId":"p-1"})] {
        let mapped = map_legacy_with_context(
            &record("task_obj", props),
            &MappingContext {
                existing_object_ids: known.clone(),
                existing_links: vec![],
            },
        )
        .expect("known canonical and snake_case targets map");
        assert_eq!(mapped.links.len(), 1);
        assert_eq!(mapped.links[0].target_object_id, "p-1");
    }

    let mapped = map_legacy_with_context(
        &task,
        &MappingContext {
            existing_object_ids: [(
                "p-1".into(),
                CanonicalIdentity::new("com.kosmos.project", "1.0.0"),
            )]
            .into_iter()
            .collect(),
            existing_links: vec![],
        },
    )
    .unwrap();
    let collision = ark_core::types::ObjectLink {
        id: mapped.links[0].id.clone(),
        source_object_id: "other".into(),
        target_object_id: "different".into(),
        link_type: "other".into(),
        created_at: "old".into(),
    };
    let error = map_legacy_with_context(
        &task,
        &MappingContext {
            existing_object_ids: [(
                "p-1".into(),
                CanonicalIdentity::new("com.kosmos.project", "1.0.0"),
            )]
            .into_iter()
            .collect(),
            existing_links: vec![collision],
        },
    )
    .expect_err("deterministic link ID collision");
    assert!(matches!(
        error,
        CompatibilityError::ConflictingFields { ref pointer, .. } if pointer == "/links"
    ));
    assert_eq!(error.source_kind(), "task_obj");
    assert_eq!(error.source_id(), "task_obj-1");
    assert_eq!(error.raw_source(), raw.as_slice());
}

#[test]
fn game_legacy_aggregates_are_local_and_not_shared_or_quarantined() {
    let mapped = map_legacy(&record(
        "game_obj",
        json!({
            "total_playtime_seconds": 42, "last_played_at": "2026-01-03", "play_count": 3
        }),
    ))
    .unwrap();
    assert_eq!(
        mapped.local_state[0].data_json["game"]["legacyAggregates"],
        json!({
            "totalPlaytimeSeconds": 42, "lastPlayedAt": "2026-01-03", "playCount": 3
        })
    );
    assert!(mapped.object.props_json["extensions"]["compatibility"]
        .get("totalPlaytimeSeconds")
        .is_none());
}

#[test]
fn recursive_forbidden_fields_are_not_silently_dropped_and_secrets_in_arrays_fail() {
    let mapped = map_legacy(&record("game_obj", json!({"custom": {"provider":"steam", "keep": 1}, "custom_array": [{"exe_path":"x", "keep":2}]}))).unwrap();
    let serialized = mapped.object.props_json.to_string();
    assert!(!serialized.contains("provider"));
    assert!(!serialized.contains("exe_path"));
    assert!(
        mapped
            .quarantine
            .iter()
            .any(|q| q.fields_json.to_string().contains("provider"))
            || mapped
                .local_state
                .iter()
                .any(|q| q.data_json.to_string().contains("exe_path"))
    );
    let secret = record("note_obj", json!({"nested": [{"api_token":"x"}]}));
    assert!(
        matches!(map_legacy(&secret), Err(CompatibilityError::SecretField { pointer, .. }) if pointer.contains("nested/0/api_token"))
    );
}

#[test]
fn non_rich_json_content_must_be_an_object() {
    for content in [json!(null), json!("text"), json!([1]), json!({"ok":true})] {
        let mut image = record("image_obj", json!({}));
        image.content = content;
        let result = map_legacy(&image);
        assert_eq!(result.is_ok(), image.content.is_object());
        if let Err(CompatibilityError::InvalidField { pointer, .. }) = result {
            assert_eq!(pointer, "/content");
        }
    }
}

#[test]
fn shared_task_parity_corpus_is_consumed_by_rust_mapper() {
    fn norm(v: serde_json::Value) -> serde_json::Value {
        match v {
            serde_json::Value::Array(a) => {
                serde_json::Value::Array(a.into_iter().map(norm).collect())
            }
            serde_json::Value::Object(o) => {
                serde_json::Value::Object(o.into_iter().map(|(k, v)| (k, norm(v))).collect())
            }
            x => x,
        }
    }
    let corpus: serde_json::Value =
        serde_json::from_str(include_str!("../../fixtures/phase3_compatibility.json")).unwrap();
    assert_eq!(
        corpus["provenance"]["mapper"],
        "ark_core::canonical_types::compatibility::map_legacy_source"
    );
    for case in corpus["cases"].as_array().unwrap() {
        let source: LegacyRecord = serde_json::from_value(case["source"].clone()).unwrap();
        let raw = serde_json::to_vec(&source).unwrap();
        let actual = match map_legacy(&source) {
            Ok(m) => {
                serde_json::json!({"ok":true,"value":{"object":{"id":m.object.id,"typeId":m.object.type_id,"typeVersion":m.object.type_version,"title":m.object.title,"contentJson":m.object.content_json,"propsJson":m.object.props_json,"createdAt":m.object.created_at,"updatedAt":m.object.updated_at,"deletedAt":m.object.deleted_at},"links":m.links,"localState":m.local_state.into_iter().map(|x| serde_json::json!({"dataJson":x.data_json})).collect::<Vec<_>>(),"quarantine":m.quarantine.into_iter().map(|x| serde_json::json!({"fieldsJson":x.fields_json})).collect::<Vec<_>>(),"rawSource":String::from_utf8(raw).unwrap()}})
            }
            Err(e) => {
                serde_json::json!({"ok":false,"error":{"code":e.code(),"sourceKind":e.source_kind(),"sourceId":e.source_id(),"pointer":e.pointer(),"rawSource":String::from_utf8_lossy(e.raw_source()).to_string()}})
            }
        };
        assert_eq!(
            norm(actual),
            norm(case["expected"].clone()),
            "case {}",
            case["name"]
        );
    }
}

