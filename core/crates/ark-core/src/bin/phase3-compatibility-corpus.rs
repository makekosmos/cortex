use ark_core::canonical_types::compatibility::{map_legacy_source, LegacyRecord, LegacySource};
use serde_json::{json, Value};
use std::fs;

fn generated(source: &LegacyRecord) -> Value {
    let raw = serde_json::to_vec(source).expect("serialize source");
    match map_legacy_source(LegacySource {
        source_kind: &source.legacy_type_id,
        source_id: &source.id,
        raw_json: &raw,
    }) {
        Ok(mapped) => json!({
            "ok": true,
            "value": {
                "object": {
                    "id": mapped.object.id,
                    "typeId": mapped.object.type_id,
                    "typeVersion": mapped.object.type_version,
                    "title": mapped.object.title,
                    "contentJson": mapped.object.content_json,
                    "propsJson": mapped.object.props_json,
                    "createdAt": mapped.object.created_at,
                    "updatedAt": mapped.object.updated_at,
                    "deletedAt": mapped.object.deleted_at
                },
                "links": mapped.links,
                "localState": mapped.local_state.into_iter().map(|state| json!({"dataJson": state.data_json})).collect::<Vec<_>>(),
                "quarantine": mapped.quarantine.into_iter().map(|item| json!({"fieldsJson": item.fields_json})).collect::<Vec<_>>(),
                "rawSource": String::from_utf8(raw).expect("source is UTF-8")
            }
        }),
        Err(error) => json!({
            "ok": false,
            "error": {
                "code": error.code(),
                "sourceKind": error.source_kind(),
                "sourceId": error.source_id(),
                "pointer": error.pointer(),
                "rawSource": String::from_utf8_lossy(error.raw_source()).to_string()
            }
        }),
    }
}

fn main() {
    let update = std::env::args().nth(1).as_deref() == Some("--update");
    let path = format!(
        "{}/tests/fixtures/phase3_compatibility.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let mut corpus: Value = serde_json::from_str(&fs::read_to_string(&path).expect("read corpus"))
        .expect("parse corpus");
    assert_eq!(corpus["provenance"]["mode"], "rust-production-mapper");
    let cases = corpus["cases"].as_array_mut().expect("corpus cases");
    for case in cases {
        let source: LegacyRecord = serde_json::from_value(case["source"].clone()).expect("source");
        let actual = generated(&source);
        if update {
            case["expected"] = actual;
        } else if actual != case["expected"] {
            panic!("corpus mismatch for {}", case["name"]);
        }
    }
    if update {
        fs::write(
            &path,
            serde_json::to_string_pretty(&corpus).expect("serialize corpus") + "\n",
        )
        .expect("write corpus");
    }
}
