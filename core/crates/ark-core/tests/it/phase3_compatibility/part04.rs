// Regenerator for the committed corpus fixture. The parity check lives in
// `part03.rs::shared_task_parity_corpus_is_consumed_by_rust_mapper`; this test
// only rewrites the `expected` values after an intentional mapper change.
// Run it explicitly with `cargo test -p ark-core --test it -- --ignored` —
// it is ignored in the normal gate so an accidental mapper change fails the
// parity check instead of silently rewriting the fixture.
#[test]
#[ignore = "fixture regenerator: run with --ignored after an intentional mapper change"]
fn regenerate_phase3_compatibility_corpus() {
    let path = format!(
        "{}/tests/fixtures/phase3_compatibility.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let mut corpus: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("read corpus"))
            .expect("parse corpus");
    assert_eq!(corpus["provenance"]["mode"], "rust-production-mapper");
    for case in corpus["cases"].as_array_mut().expect("corpus cases") {
        let source: LegacyRecord = serde_json::from_value(case["source"].clone()).expect("source");
        let raw = serde_json::to_vec(&source).expect("serialize source");
        case["expected"] = match map_legacy(&source) {
            Ok(m) => {
                serde_json::json!({"ok":true,"value":{"object":{"id":m.object.id,"typeId":m.object.type_id,"typeVersion":m.object.type_version,"title":m.object.title,"contentJson":m.object.content_json,"propsJson":m.object.props_json,"createdAt":m.object.created_at,"updatedAt":m.object.updated_at,"deletedAt":m.object.deleted_at},"links":m.links,"localState":m.local_state.into_iter().map(|x| serde_json::json!({"dataJson":x.data_json})).collect::<Vec<_>>(),"quarantine":m.quarantine.into_iter().map(|x| serde_json::json!({"fieldsJson":x.fields_json})).collect::<Vec<_>>(),"rawSource":String::from_utf8(raw).unwrap()}})
            }
            Err(e) => {
                serde_json::json!({"ok":false,"error":{"code":e.code(),"sourceKind":e.source_kind(),"sourceId":e.source_id(),"pointer":e.pointer(),"rawSource":String::from_utf8_lossy(e.raw_source()).to_string()}})
            }
        };
    }
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&corpus).expect("serialize corpus") + "\n",
    )
    .expect("write corpus");
}
