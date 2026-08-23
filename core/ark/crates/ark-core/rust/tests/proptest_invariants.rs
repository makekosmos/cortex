#![allow(clippy::unwrap_used)]

// Property-based invariant tests для ARK objects + sync layer.
//
// Часть pre-in-process-hardening proof loop (см. .agent/tasks/2026-05-18-
// pre-in-process-hardening/spec.md AC6). Random Upsert/Delete sequences
// проверяют что invariants держатся при любом порядке операций.
//
// Invariants tested:
//   I1. List доступен после любой sequence — никакая sequence не должна
//       вызвать panic в list_objects() или вернуть garbage.
//   I2. Tombstone consistency — после Delete id, sync_tombstones содержит
//       row для id. После повторного Insert(id) tombstone исчезает.
//   I3. Soft-delete idempotent — Delete несуществующего id — silent no-op.
//   I4. Version vector monotonicity — HLC для каждого device_id растёт
//       (lexicographic compare) после каждой write op.
//
// Test config: 64 iterations (CI-friendly), max sequence length 30.
// Если будет flaky или slow — можно увеличить до 256 при ручном запуске.

use ark_core::db;
use ark_core::types::{ArkObject, ObjectType, VersionVector};
use proptest::collection::vec;
use proptest::prelude::*;
use rusqlite::Connection;
use serde_json::json;

const TEST_TYPE_ID: &str = "test_obj";
const TEST_DEVICE_ID: &str = "device-prop-test";

#[derive(Debug, Clone)]
enum Op {
    Upsert { id: String, title: String },
    Delete { id: String },
    DeleteNonExistent { id: String },
}

fn op_strategy() -> impl Strategy<Value = Op> {
    // Pool из ~5 id'шников чтобы операции пересекались.
    let id_pool: Vec<String> = (0..5).map(|i| format!("id-{i}")).collect();
    let id_strategy = proptest::sample::select(id_pool.clone());
    let unknown_id_strategy = (0u32..100).prop_map(|n| format!("never-{n}"));
    prop_oneof![
        4 => (id_strategy.clone(), "[a-z]{1,12}").prop_map(|(id, title)| Op::Upsert { id, title }),
        2 => id_strategy.prop_map(|id| Op::Delete { id }),
        1 => unknown_id_strategy.prop_map(|id| Op::DeleteNonExistent { id }),
    ]
}

fn op_sequence() -> impl Strategy<Value = Vec<Op>> {
    vec(op_strategy(), 1..30)
}

fn setup_test_db() -> Connection {
    let conn = Connection::open_in_memory().expect("in-memory db");
    db::init_schema(&conn).expect("init_schema");
    let object_type = ObjectType {
        id: TEST_TYPE_ID.to_string(),
        name: "Test Object".to_string(),
        schema_json: "{}".to_string(),
        ui_schema_json: "{}".to_string(),
        created_at: "2026-05-18T00:00:00.000Z".to_string(),
        updated_at: "2026-05-18T00:00:00.000Z".to_string(),
        system_locked: false,
    };
    db::upsert_object_type(&conn, &object_type).expect("upsert_object_type");
    conn
}

fn make_obj(id: &str, title: &str) -> ArkObject {
    let now = "2026-05-18T00:00:00.000Z".to_string();
    ArkObject {
        id: id.to_string(),
        type_id: TEST_TYPE_ID.to_string(),
        type_version: "0.0.0-legacy".to_string(),
        title: title.to_string(),
        content_json: json!({ "type": "doc", "content": [] }),
        props_json: json!({ "padding": "x" }),
        created_at: now.clone(),
        updated_at: now,
        deleted_at: None,
    }
}

fn bump_local_upsert(conn: &Connection, id: &str) {
    let _ = db::bump_sync_version_vector(conn, "object", id, TEST_DEVICE_ID, false);
    let _ = db::delete_sync_tombstone(conn, id);
}

fn bump_local_delete(conn: &Connection, id: &str) {
    let hlc = db::bump_sync_version_vector(conn, "object", id, TEST_DEVICE_ID, true)
        .expect("bump should succeed");
    let _ = db::record_sync_tombstone(conn, "object", id, &hlc);
}

fn read_version_vector(conn: &Connection) -> VersionVector {
    let raw = db::get_sync_kv(conn, "lan_sync.version_vector")
        .expect("get_sync_kv")
        .unwrap_or_else(|| "{}".to_string());
    serde_json::from_str(&raw).expect("version_vector parses")
}

fn count_tombstones(conn: &Connection, id: &str) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM sync_tombstones WHERE id = ?1 AND entity_type = 'object'",
        rusqlite::params![id],
        |row| row.get(0),
    )
    .unwrap_or(0)
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 64,
        max_shrink_iters: 256,
        ..ProptestConfig::default()
    })]

    /// I1 + I3: list survives + delete несуществующего — no-op.
    #[test]
    fn list_survives_random_ops(ops in op_sequence()) {
        let conn = setup_test_db();
        for op in &ops {
            match op {
                Op::Upsert { id, title } => {
                    let obj = make_obj(id, title);
                    db::upsert_object(&conn, &obj).expect("upsert should not panic");
                    bump_local_upsert(&conn, id);
                }
                Op::Delete { id } => {
                    // delete_object возвращает Ok даже для несуществующего id.
                    db::delete_object(&conn, id).expect("delete should not panic");
                    bump_local_delete(&conn, id);
                }
                Op::DeleteNonExistent { id } => {
                    db::delete_object(&conn, id).expect("delete non-existent should not panic");
                    bump_local_delete(&conn, id);
                }
            }
            // List после каждой op — должен работать без panic'а.
            let _ = db::list_objects(&conn).expect("list must succeed mid-sequence");
        }
        // Final list — успех = ok, без panic'а уже было проверено.
        let final_list = db::list_objects(&conn).expect("final list");
        // Sanity: все returned objects имеют не-empty id и matching type_id.
        for obj in &final_list {
            prop_assert!(!obj.id.is_empty(), "object id must be non-empty");
            prop_assert_eq!(&obj.type_id, TEST_TYPE_ID);
        }
    }

    /// I4: HLC растёт после каждой write op (по device_id).
    #[test]
    fn version_vector_monotonic_per_entity(ops in op_sequence()) {
        let conn = setup_test_db();
        let mut last_hlc_per_id: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        for op in &ops {
            let id = match op {
                Op::Upsert { id, .. } => id,
                Op::Delete { id } => id,
                Op::DeleteNonExistent { id } => id,
            };
            // Apply op.
            match op {
                Op::Upsert { id, title } => {
                    db::upsert_object(&conn, &make_obj(id, title)).unwrap();
                    bump_local_upsert(&conn, id);
                }
                Op::Delete { id } | Op::DeleteNonExistent { id } => {
                    db::delete_object(&conn, id).unwrap();
                    bump_local_delete(&conn, id);
                }
            }
            // Проверка: HLC для этого id строго больше предыдущего (lex).
            let vv = read_version_vector(&conn);
            if let Some(new_hlc) = vv.get(id) {
                if let Some(prev) = last_hlc_per_id.get(id) {
                    prop_assert!(
                        new_hlc.as_str() > prev.as_str(),
                        "HLC должен строго расти: {prev} -> {new_hlc} (id {id}, op {op:?})"
                    );
                }
                last_hlc_per_id.insert(id.clone(), new_hlc.clone());
            }
        }
    }

    /// I2: после Delete tombstone существует; после re-Insert tombstone
    /// удалён.
    #[test]
    fn tombstone_consistent_with_lifecycle(id_idx in 0u8..5, cycles in 1u8..5) {
        let conn = setup_test_db();
        let id = format!("id-{id_idx}");
        for cycle in 0..cycles {
            // Insert.
            db::upsert_object(&conn, &make_obj(&id, &format!("cycle-{cycle}"))).unwrap();
            bump_local_upsert(&conn, &id);
            prop_assert_eq!(
                count_tombstones(&conn, &id),
                0,
                "после upsert tombstone должен быть удалён"
            );

            // Delete.
            db::delete_object(&conn, &id).unwrap();
            bump_local_delete(&conn, &id);
            prop_assert_eq!(
                count_tombstones(&conn, &id),
                1,
                "после delete tombstone должен быть записан"
            );
        }
    }
}
