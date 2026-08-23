use ark_core::canonical_types::compatibility::{
    map_legacy_source, map_legacy_with_context as map_legacy_with_context_source,
    CanonicalIdentity, CompatibilityError, LegacyRecord, LegacySource, MappingContext,
};
use serde_json::json;

fn map_legacy(
    record: &LegacyRecord,
) -> Result<ark_core::canonical_types::compatibility::MappedRecord, CompatibilityError> {
    let raw = serde_json::to_vec(record).unwrap();
    map_legacy_source(LegacySource {
        source_kind: &record.legacy_type_id,
        source_id: &record.id,
        raw_json: &raw,
    })
}

fn map_legacy_with_context(
    record: &LegacyRecord,
    context: &MappingContext,
) -> Result<ark_core::canonical_types::compatibility::MappedRecord, CompatibilityError> {
    let raw = serde_json::to_vec(record).unwrap();
    map_legacy_with_context_source(
        LegacySource {
            source_kind: &record.legacy_type_id,
            source_id: &record.id,
            raw_json: &raw,
        },
        context,
    )
}

fn record(alias: &str, props: serde_json::Value) -> LegacyRecord {
    LegacyRecord {
        id: format!("{alias}-1"),
        legacy_type_id: alias.into(),
        title: "Preserved title".into(),
        content: json!({"type":"doc","content":[{"type":"paragraph"}]}),
        props,
        created_at: "2026-01-01T00:00:00Z".into(),
        updated_at: "2026-01-02T00:00:00Z".into(),
        deleted_at: None,
    }
}

#[test]
fn all_nine_aliases_map_to_exact_canonical_ids_and_version() {
    let aliases = [
        ("note_obj", "com.kosmos.note"),
        ("task_obj", "com.kosmos.task"),
        ("project_obj", "com.kosmos.project"),
        ("tag_obj", "com.kosmos.tag"),
        ("person_obj", "com.kosmos.person"),
        ("image_obj", "com.kosmos.image"),
        ("time_entry_obj", "com.kosmos.time-entry"),
        ("game_obj", "com.kosmos.game"),
        ("book_obj", "com.kosmos.book"),
    ];
    for (alias, canonical) in aliases {
        let props = if alias == "time_entry_obj" {
            json!({"started_at":"2026-01-01T00:00:00Z"})
        } else {
            json!({})
        };
        let mapped = map_legacy(&record(alias, props)).unwrap();
        assert_eq!(mapped.object.type_id, canonical);
        assert_eq!(mapped.object.type_version, "1.0.0");
        assert_eq!(mapped.object.id, format!("{alias}-1"));
        assert_eq!(mapped.object.title, "Preserved title");
        assert_eq!(
            mapped.object.content_json,
            json!({"type":"doc","content":[{"type":"paragraph"}]})
        );
    }
}

#[test]
fn exact_source_and_play_status_rules_are_fail_closed() {
    let mut time = record(
        "time_entry_obj",
        json!({"started_at":"2026-01-01T00:00:00Z"}),
    );
    assert_eq!(
        map_legacy(&time).unwrap().object.props_json["source"],
        "manual"
    );
    time.props["source"] = json!("other");
    assert!(matches!(
        map_legacy(&time),
        Err(CompatibilityError::InvalidField { .. })
    ));

    for value in [
        json!(null),
        json!("not_started"),
        json!("in_progress"),
        json!("completed"),
        json!("abandoned"),
    ] {
        let game = record("game_obj", json!({"play_status": value}));
        let output = map_legacy(&game).unwrap();
        assert_eq!(
            output.object.props_json["playStatus"],
            if value.is_null() {
                json!(null)
            } else {
                json!(value
                    .as_str()
                    .unwrap()
                    .replace("not_started", "notStarted")
                    .replace("in_progress", "inProgress"))
            }
        );
    }
    let bad = record("game_obj", json!({"play_status":"unknown"}));
    assert!(matches!(
        map_legacy(&bad),
        Err(CompatibilityError::InvalidField { .. })
    ));
}

#[test]
fn preserves_relations_and_local_quarantine_without_side_effects() {
    let mapped = map_legacy(&record(
        "task_obj",
        json!({
            "project_id":"project-1", "tag_ids":["tag-2", "tag-1"],
            "exe_path":"C:\\Games\\game.exe", "source_app":"steam"
        }),
    ))
    .unwrap();
    assert_eq!(
        mapped
            .links
            .iter()
            .map(|l| l.link_type.as_str())
            .collect::<Vec<_>>(),
        ["project", "tag", "tag"]
    );
    assert_eq!(
        mapped.local_state[0].data_json["planning"]["exePath"],
        "C:\\Games\\game.exe"
    );
    assert_eq!(mapped.quarantine[0].fields_json["sourceApp"], "steam");
    assert!(!mapped.object.props_json.to_string().contains("project_id"));
}

#[test]
fn detects_conflicts_secrets_and_rich_text_loss_risk() {
    let conflict = record("task_obj", json!({"scheduledAt":"a", "scheduled_date":"b"}));
    assert!(matches!(
        map_legacy(&conflict),
        Err(CompatibilityError::ConflictingFields { .. })
    ));
    let secret = record("note_obj", json!({"api_token":"secret"}));
    assert!(matches!(
        map_legacy(&secret),
        Err(CompatibilityError::SecretField { .. })
    ));
    let rich = record("note_obj", json!({"description":"x"}));
    let mut rich = rich;
    rich.content = json!({"type":"doc","content":[{"type":"unknown"}]});
    assert!(matches!(
        map_legacy(&rich),
        Err(CompatibilityError::DataLossRisk { .. })
    ));
}

#[test]
fn image_and_book_sources_are_bounded_to_local_or_quarantine() {
    let local = map_legacy(&record("image_obj", json!({"source_path":"/tmp/a.png"}))).unwrap();
    assert_eq!(
        local.local_state[0].data_json["image"]["sourcePath"],
        "/tmp/a.png"
    );
    let remote = map_legacy(&record(
        "book_obj",
        json!({"cover_image":"https://example.invalid/a"}),
    ))
    .unwrap();
    assert_eq!(
        remote.quarantine[0].fields_json["coverImage"],
        "https://example.invalid/a"
    );
}

#[test]
fn raw_source_and_context_preserve_malformed_and_existing_link_semantics() {
    let raw = br#"{"#;
    let error = map_legacy_source(LegacySource {
        source_kind: "note_obj",
        source_id: "note-raw-1",
        raw_json: raw,
    })
    .expect_err("malformed source must be rejected");
    assert_eq!(error.code(), "MALFORMED_JSON");
    assert_eq!(error.source_kind(), "note_obj");
    assert_eq!(error.source_id(), "note-raw-1");
    assert_eq!(error.pointer(), "");
    assert_eq!(error.raw_source(), raw);
    let valid_raw = br#"{"id":"note-raw-2","legacy_type_id":"note_obj","title":"Raw","content":{"type":"doc","content":[{"type":"paragraph"}]},"props":{},"created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-02T00:00:00Z","deleted_at":null}"#;
    let mapped = map_legacy_source(LegacySource {
        source_kind: "note_obj",
        source_id: "note-raw-2",
        raw_json: valid_raw,
    })
    .expect("valid raw source");
    assert_eq!(mapped.raw_source.as_deref(), Some(valid_raw.as_slice()));
    let book = record(
        "book_obj",
        json!({"cover_image":"https://example.invalid/a"}),
    );
    let mapped = map_legacy_with_context(
        &book,
        &ark_core::canonical_types::compatibility::MappingContext {
            existing_object_ids: std::collections::BTreeMap::new(),
            existing_links: vec![],
        },
    )
    .unwrap();
    assert!(mapped
        .links
        .iter()
        .all(|link| link.target_object_id != "https://example.invalid/a"));
}

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
        serde_json::from_str(include_str!("fixtures/phase3_compatibility.json")).unwrap();
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
