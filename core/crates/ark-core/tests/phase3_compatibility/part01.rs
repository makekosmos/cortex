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