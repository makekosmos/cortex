
    #[test]
    fn batch_upsert_todos_nests_in_outer_transaction() {
        let conn = setup_db();
        // РћС‚РєСЂС‹РІР°РµРј РІРЅРµС€РЅСЋСЋ С‚СЂР°РЅР·Р°РєС†РёСЋ вЂ” РёРјРёС‚РёСЂСѓРµРј РІС‹Р·РѕРІ РёР· with_write_tx.
        conn.execute_batch("BEGIN IMMEDIATE").unwrap();
        let todos = vec![
            make_todo("bt1", "Nested todo A"),
            make_todo("bt2", "Nested todo B"),
        ];
        // Р”Рѕ Р¤Р°Р·С‹ B РїР°РґР°РµС‚: "cannot start a transaction within a transaction".
        // РџРѕСЃР»Рµ Р¤Р°Р·С‹ B (SAVEPOINT) РґРѕР»Р¶РЅРѕ РїСЂРѕР№С‚Рё Р±РµР· РѕС€РёР±РєРё.
        batch_upsert_todos(&conn, &todos)
            .expect("batch_upsert_todos РґРѕР»Р¶РµРЅ СЂР°Р±РѕС‚Р°С‚СЊ РІРЅСѓС‚СЂРё РІРЅРµС€РЅРµР№ С‚СЂР°РЅР·Р°РєС†РёРё");
        conn.execute_batch("COMMIT").unwrap();
        // РџСЂРѕРІРµСЂСЏРµРј, С‡С‚Рѕ todo РґРµР№СЃС‚РІРёС‚РµР»СЊРЅРѕ Р·Р°РїРёСЃР°РЅС‹.
        let data = load_all(&conn).unwrap();
        assert_eq!(data.todos.len(), 2, "РѕР±Р° todo РґРѕР»Р¶РЅС‹ Р±С‹С‚СЊ Р·Р°РїРёСЃР°РЅС‹");
    }

    #[test]
    fn upsert_object_type_nests_in_outer_transaction() {
        let conn = setup_db();
        let ot = make_object_type("ot-nested", "РўРёРї РІР»РѕР¶РµРЅРЅС‹Р№");
        // РћС‚РєСЂС‹РІР°РµРј РІРЅРµС€РЅСЋСЋ С‚СЂР°РЅР·Р°РєС†РёСЋ вЂ” РёРјРёС‚РёСЂСѓРµРј РІС‹Р·РѕРІ РёР· with_write_tx.
        conn.execute_batch("BEGIN IMMEDIATE").unwrap();
        // Р”Рѕ Р¤Р°Р·С‹ B РїР°РґР°РµС‚ С‡РµСЂРµР· replay_pending_for_type в†’ BEGIN IMMEDIATE:
        // "cannot start a transaction within a transaction".
        // РџРѕСЃР»Рµ Р¤Р°Р·С‹ B (SAVEPOINT) РґРѕР»Р¶РЅРѕ РїСЂРѕР№С‚Рё Р±РµР· РѕС€РёР±РєРё.
        upsert_object_type(&conn, &ot)
            .expect("upsert_object_type РґРѕР»Р¶РµРЅ СЂР°Р±РѕС‚Р°С‚СЊ РІРЅСѓС‚СЂРё РІРЅРµС€РЅРµР№ С‚СЂР°РЅР·Р°РєС†РёРё");
        conn.execute_batch("COMMIT").unwrap();
        // РџСЂРѕРІРµСЂСЏРµРј, С‡С‚Рѕ С‚РёРї Р·Р°РїРёСЃР°РЅ.
        let types = list_object_types(&conn).unwrap();
        assert!(
            types.iter().any(|t| t.id == "ot-nested"),
            "object_type РґРѕР»Р¶РµРЅ Р±С‹С‚СЊ Р·Р°РїРёСЃР°РЅ"
        );
    }

    #[test]
    fn compact_usage_span_is_idempotent_and_dictionary_encoded() {
        let conn = setup_db();
        let mut write = UsageSpanWrite {
            device_id: "device-compact".to_string(),
            started_at_unix: 1_767_225_600,
            ended_at_unix: 1_767_225_630,
            tracked_app_id: "browser".to_string(),
            window_title: Some("Facebook".to_string()),
            flags: 0,
            updated_at: "2026-01-01T00:00:30.000Z".to_string(),
        };
        upsert_usage_span(&conn, &write).unwrap();
        write.ended_at_unix += 30;
        upsert_usage_span(&conn, &write).unwrap();

        let day = load_usage_day(&conn, "usage-day:device-compact:2026-01-01")
            .unwrap()
            .unwrap();
        assert_eq!(day.payload_json["a"], json!(["browser"]));
        assert_eq!(day.payload_json["t"], json!(["Facebook"]));
        assert_eq!(day.payload_json["s"], json!([[0, 60, 0, 0, 0]]));
        assert_eq!(
            get_usage_title_total(&conn, "face").unwrap(),
            UsageTitleTotal {
                active_seconds: 60,
                idle_seconds: 0,
            }
        );
    }

    #[test]
    fn compact_usage_span_splits_at_utc_midnight_and_syncs() {
        let source = setup_db();
        let write = UsageSpanWrite {
            device_id: "device-midnight".to_string(),
            started_at_unix: 1_767_311_990,
            ended_at_unix: 1_767_312_010,
            tracked_app_id: "reader".to_string(),
            window_title: Some("Book".to_string()),
            flags: 1,
            updated_at: "2026-01-02T00:00:10.000Z".to_string(),
        };
        let days = upsert_usage_span(&source, &write).unwrap();
        assert_eq!(days.len(), 2);
        ensure_usage_sequence_migrated(&source, "source-device").unwrap();

        let entities = SqliteStorageBackend::collect_entities_blocking(
            &source,
            &VersionVector::new(),
            "source-device",
        );
        let usage_days = entities
            .iter()
            .filter(|entity| entity.entity_type == "usage_day")
            .collect::<Vec<_>>();
        assert_eq!(usage_days.len(), 2);

        let peer = setup_db();
        for entity in usage_days {
            SqliteStorageBackend::apply_entity_blocking(&peer, entity).unwrap();
        }
        assert_eq!(
            peer.query_row("SELECT COUNT(*) FROM usage_days", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            2
        );
    }

    #[test]
    fn usage_sequence_cursor_waits_for_missing_entries() {
        let conn = setup_db();

        record_usage_sequence(
            &conn,
            "usage_day",
            "day-2",
            "peer-device",
            2,
            "2026-01-02T00:00:00.000Z:000002:peer-device",
            false,
        )
        .unwrap();
        let vector: VersionVector =
            serde_json::from_str(&get_sync_kv(&conn, VERSION_VECTOR_KEY).unwrap().unwrap())
                .unwrap();
        assert_eq!(vector.get("@usage:peer-device"), Some(&"0".to_string()));

        record_usage_sequence(
            &conn,
            "usage_day",
            "day-1",
            "peer-device",
            1,
            "2026-01-01T00:00:00.000Z:000001:peer-device",
            false,
        )
        .unwrap();
        let vector: VersionVector =
            serde_json::from_str(&get_sync_kv(&conn, VERSION_VECTOR_KEY).unwrap().unwrap())
                .unwrap();
        assert_eq!(vector.get("@usage:peer-device"), Some(&"2".to_string()));
    }

    #[test]
    fn local_usage_versions_use_one_device_cursor() {
        let conn = setup_db();
        let first =
            bump_sync_version_vector(&conn, "usage_day", "day-a", "local-device", false).unwrap();
        let second =
            bump_sync_version_vector(&conn, "usage_day", "day-b", "local-device", false).unwrap();

        let vector: VersionVector =
            serde_json::from_str(&get_sync_kv(&conn, VERSION_VECTOR_KEY).unwrap().unwrap())
                .unwrap();
        assert_eq!(vector.get("@usage:local-device"), Some(&"2".to_string()));
        assert!(!vector.contains_key("day-a"));
        assert!(!vector.contains_key("day-b"));
        assert!(HLC::is_newer(&second, &first));
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM usage_sync_log", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
            2
        );
    }

    #[test]
    fn legacy_usage_replay_keeps_one_synthetic_sequence() {
        let conn = setup_db();
        let day = UsageDay {
            id: "usage-day:legacy:2026-01-01".to_string(),
            device_id: "legacy".to_string(),
            day: "2026-01-01".to_string(),
            payload_json: json!({"a": ["browser"], "t": ["Example"], "s": [[0, 30, 0, 0, 0]]}),
            updated_at: "2026-01-01T00:00:30.000Z".to_string(),
        };
        let mut data = serde_json::to_value(&day)
            .unwrap()
            .as_object()
            .unwrap()
            .clone();
        data.remove("id");
        let entity = SyncEntity {
            entity_type: "usage_day".to_string(),
            id: day.id.clone(),
            data,
            hlc: "2026-01-01T00:00:30.000Z:000007:old-peer".to_string(),
            deleted: None,
            origin_device_id: None,
            origin_seq: None,
        };

        SqliteStorageBackend::apply_entity_blocking(&conn, &entity).unwrap();
        SqliteStorageBackend::apply_entity_blocking(&conn, &entity).unwrap();

        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM usage_sync_log", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
            1
        );
        let vector: VersionVector =
            serde_json::from_str(&get_sync_kv(&conn, VERSION_VECTOR_KEY).unwrap().unwrap())
                .unwrap();
        assert_eq!(
            vector.get("@usage:legacy:old-peer").map(String::as_str),
            Some("1")
        );
        assert!(!vector.contains_key(&day.id));
    }
