
    fn phase2_make_object_type(id: &str, name: &str) -> ObjectType {
        ObjectType {
            id: id.to_string(),
            name: name.to_string(),
            schema_json: "{}".to_string(),
            ui_schema_json: "{}".to_string(),
            created_at: "2026-01-01T00:00:00.000Z".to_string(),
            updated_at: "2026-01-01T00:00:00.000Z".to_string(),
            system_locked: false,
        }
    }

    fn phase2_make_sync_entity_object(id: &str, type_id: &str, title: &str) -> SyncEntity {
        let mut data = serde_json::Map::new();
        data.insert("typeId".to_string(), Value::String(type_id.to_string()));
        data.insert(
            "typeVersion".to_string(),
            Value::String("0.0.0-legacy".to_string()),
        );
        data.insert("title".to_string(), Value::String(title.to_string()));
        data.insert("contentJson".to_string(), json!({}));
        data.insert("propsJson".to_string(), json!({}));
        data.insert(
            "createdAt".to_string(),
            Value::String("2026-01-01T00:00:00.000Z".to_string()),
        );
        data.insert(
            "updatedAt".to_string(),
            Value::String("2026-01-01T00:00:00.000Z".to_string()),
        );
        SyncEntity {
            entity_type: "object".to_string(),
            id: id.to_string(),
            data,
            hlc: "2026-01-01T00:00:00.000Z:000001:test".to_string(),
            deleted: None,
            origin_device_id: None,
            origin_seq: None,
        }
    }

    #[test]
    fn phase2_is_object_type_known_false_when_missing() {
        let conn = setup_db();
        assert!(!is_object_type_known(&conn, "nonexistent").unwrap());
    }

    #[test]
    fn phase2_is_object_type_known_true_after_upsert() {
        let conn = setup_db();
        let object_type = phase2_make_object_type("type_x", "Type X");
        upsert_object_type(&conn, &object_type).unwrap();
        assert!(is_object_type_known(&conn, "type_x").unwrap());
    }

    #[test]
    fn phase2_insert_pending_object_persists_payload() {
        let conn = setup_db();
        let entity = phase2_make_sync_entity_object("obj-1", "future_type", "Pending object");
        insert_pending_object(&conn, &entity, "future_type").unwrap();
        assert_eq!(count_pending_for_type(&conn, "future_type").unwrap(), 1);
        assert_eq!(count_pending_for_type(&conn, "other_type").unwrap(), 0);
    }

    #[test]
    fn phase2_insert_pending_overwrites_same_id() {
        // Р•СЃР»Рё sync РїСЂРёРЅРѕСЃРёС‚ РѕР±РЅРѕРІР»С‘РЅРЅСѓСЋ version С‚РѕРіРѕ Р¶Рµ object'Р°, REPLACE'РёС‚,
        // РЅРµ РґСѓР±Р»РёСЂСѓРµС‚.
        let conn = setup_db();
        let e1 = phase2_make_sync_entity_object("obj-1", "future_type", "First");
        let e2 = phase2_make_sync_entity_object("obj-1", "future_type", "Second");
        insert_pending_object(&conn, &e1, "future_type").unwrap();
        insert_pending_object(&conn, &e2, "future_type").unwrap();
        assert_eq!(count_pending_for_type(&conn, "future_type").unwrap(), 1);
    }

    #[test]
    fn phase2_replay_runs_when_type_appears() {
        let conn = setup_db();
        // 1. Pending object РґР»СЏ С‚РёРїР°, РєРѕС‚РѕСЂС‹Р№ РµС‰С‘ РЅРµ СЃСѓС‰РµСЃС‚РІСѓРµС‚.
        let entity = phase2_make_sync_entity_object("obj-1", "type_late", "Awaiting type");
        insert_pending_object(&conn, &entity, "type_late").unwrap();
        assert_eq!(count_pending_for_type(&conn, "type_late").unwrap(), 1);

        // Object table РґРѕР»Р¶РЅР° Р±С‹С‚СЊ РїСѓСЃС‚РѕР№.
        let objects_before = list_objects(&conn).unwrap();
        assert_eq!(objects_before.len(), 0);

        // 2. РЎРѕР·РґР°С‘Рј С‚РёРї вЂ” РґРѕР»Р¶РµРЅ СЃСЂР°Р±РѕС‚Р°С‚СЊ auto-replay.
        let object_type = phase2_make_object_type("type_late", "Late Type");
        upsert_object_type(&conn, &object_type).unwrap();

        // 3. Pending РѕС‡РёС‰РµРЅ, object РјР°С‚РµСЂРёР°Р»РёР·РѕРІР°РЅ.
        assert_eq!(count_pending_for_type(&conn, "type_late").unwrap(), 0);
        let objects_after = list_objects(&conn).unwrap();
        assert_eq!(objects_after.len(), 1);
        assert_eq!(objects_after[0].id, "obj-1");
        assert_eq!(objects_after[0].title, "Awaiting type");
        assert_eq!(objects_after[0].type_id, "type_late");
    }

    #[test]
    fn phase2_replay_handles_multiple_pending_same_type() {
        let conn = setup_db();
        for i in 0..5 {
            let e = phase2_make_sync_entity_object(
                &format!("obj-{i}"),
                "batch_type",
                &format!("Obj {i}"),
            );
            insert_pending_object(&conn, &e, "batch_type").unwrap();
        }
        assert_eq!(count_pending_for_type(&conn, "batch_type").unwrap(), 5);

        upsert_object_type(&conn, &phase2_make_object_type("batch_type", "Batch")).unwrap();

        assert_eq!(count_pending_for_type(&conn, "batch_type").unwrap(), 0);
        assert_eq!(list_objects(&conn).unwrap().len(), 5);
    }

    #[test]
    fn phase2_replay_only_targets_matching_type() {
        let conn = setup_db();
        insert_pending_object(
            &conn,
            &phase2_make_sync_entity_object("obj-a", "type_a", "A"),
            "type_a",
        )
        .unwrap();
        insert_pending_object(
            &conn,
            &phase2_make_sync_entity_object("obj-b", "type_b", "B"),
            "type_b",
        )
        .unwrap();

        upsert_object_type(&conn, &phase2_make_object_type("type_a", "Type A")).unwrap();

        // РўРѕР»СЊРєРѕ obj-a replayed; obj-b РІСЃС‘ РµС‰С‘ РІ pending.
        assert_eq!(count_pending_for_type(&conn, "type_a").unwrap(), 0);
        assert_eq!(count_pending_for_type(&conn, "type_b").unwrap(), 1);
        let objects = list_objects(&conn).unwrap();
        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0].id, "obj-a");
    }

    // --- Hardening (2026-05-18) ---

    #[test]
    fn check_integrity_passes_on_fresh_db() {
        let conn = Connection::open_in_memory().unwrap();
        check_integrity(&conn).expect("РїСѓСЃС‚Р°СЏ DB РґРѕР»Р¶РЅР° РїСЂРѕС…РѕРґРёС‚СЊ integrity_check");
    }

    #[test]
    fn check_integrity_passes_after_init_schema() {
        let conn = setup_db();
        check_integrity(&conn).expect("DB РїРѕСЃР»Рµ init_schema РґРѕР»Р¶РЅР° Р±С‹С‚СЊ С†РµР»РѕСЃС‚РЅРѕР№");
    }

    #[test]
    fn init_schema_fails_on_corrupted_db() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        // РЎРѕР·РґР°С‘Рј РІР°Р»РёРґРЅСѓСЋ DB Рё РЅР°РїРѕР»РЅСЏРµРј РґР°РЅРЅС‹РјРё (РЅСѓР¶РЅРѕ в‰Ґ 1 data page С‡С‚РѕР±С‹
        // РїРѕРІСЂРµРґРёС‚СЊ РЅРµ header).
        {
            let conn = open_db(path.to_str().unwrap()).unwrap();
            init_schema(&conn).unwrap();
            let object_type = make_object_type("corruption_fixture_type", "Note");
            upsert_object_type(&conn, &object_type).unwrap();
            for i in 0..100 {
                let mut obj = make_object(
                    &format!("obj-{i}"),
                    "corruption_fixture_type",
                    &format!("Title {i}"),
                );
                obj.props_json = json!({ "n": i, "padding": "x".repeat(200) });
                upsert_object(&conn, &obj).unwrap();
            }
        }
        // РџРѕСЂС‚РёРј data pages (offset 8192+, РїРѕСЃР»Рµ header page Рё schema page) вЂ”
        // SQLite header РѕСЃС‚Р°РЅРµС‚СЃСЏ РІР°Р»РёРґРЅС‹Рј, integrity_check РѕР±РЅР°СЂСѓР¶РёС‚
        // РїРѕРІСЂРµР¶РґС‘РЅРЅС‹Рµ btree pages.
        {
            use std::io::{Seek, SeekFrom, Write};
            let mut file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
            file.seek(SeekFrom::Start(8192)).unwrap();
            file.write_all(&[0xFF; 4096]).unwrap();
            file.flush().unwrap();
        }
        // РћС‚РєСЂС‹РІР°РµРј СЃРЅРѕРІР°. open_db РјРѕР¶РµС‚ РїСЂРѕР№С‚Рё (header РёРЅС‚Р°РєС‚РµРЅ) РёР»Рё fail
        // РЅР° PRAGMA journal_mode. Р•СЃР»Рё РѕС‚РєСЂС‹С‚ вЂ” init_schema fail'РёС‚ РЅР°
        // integrity_check. Р›СЋР±РѕР№ РїСѓС‚СЊ вЂ” fail-loud.
        let open_result = open_db(path.to_str().unwrap());
        if let Ok(conn) = open_result {
            let result = init_schema(&conn);
            assert!(
                result.is_err(),
                "init_schema РґРѕР»Р¶РµРЅ fail РїСЂРё corrupted DB; СЂРµР·СѓР»СЊС‚Р°С‚: {result:?}"
            );
            let err = result.unwrap_err();
            assert!(
                err.contains("integrity")
                    || err.contains("corruption")
                    || err.contains("malformed"),
                "РѕС€РёР±РєР° РґРѕР»Р¶РЅР° СѓРїРѕРјРёРЅР°С‚СЊ integrity/corruption/malformed, РїРѕР»СѓС‡РёР»Рё: {err}"
            );
        }
        // else: open_db СѓР¶Рµ fail'РёР» вЂ” С‚РѕР¶Рµ acceptable fail-loud path.
    }

    #[test]
    fn backup_to_file_creates_valid_copy() {
        let conn = setup_db();
        // РџРѕР»РѕР¶РёРј test РґР°РЅРЅС‹Рµ.
        let object_type = make_object_type("backup_fixture_type", "Note");
        upsert_object_type(&conn, &object_type).unwrap();
        let mut object = make_object("obj-backup", "backup_fixture_type", "Backup test");
        object.props_json = json!({"tag": "test"});
        upsert_object(&conn, &object).unwrap();

        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("ark.db.backup");
        let dest_str = dest.to_str().unwrap();
        backup_to_file(&conn, dest_str).expect("backup РґРѕР»Р¶РµРЅ РїСЂРѕР№С‚Рё");

        assert!(dest.exists(), "С„Р°Р№Р» backup'Р° РґРѕР»Р¶РµРЅ СЃСѓС‰РµСЃС‚РІРѕРІР°С‚СЊ");
        let backup_conn = open_db(dest_str).unwrap();
        let objects = list_objects(&backup_conn).unwrap();
        assert_eq!(
            objects.len(),
            1,
            "backup РґРѕР»Р¶РµРЅ СЃРѕРґРµСЂР¶Р°С‚СЊ РѕСЂРёРіРёРЅР°Р»СЊРЅС‹Р№ РѕР±СЉРµРєС‚"
        );
        assert_eq!(objects[0].id, "obj-backup");
        assert_eq!(objects[0].title, "Backup test");
    }

    #[test]
    fn backup_to_file_chunked_creates_valid_copy_via_separate_connection() {
        let tmp = tempfile::tempdir().unwrap();
        let src_path = tmp.path().join("ark.db");
        let src_str = src_path.to_str().unwrap();
        {
            // Р¤Р°Р№Р»РѕРІС‹Р№ source: chunked backup РѕС‚РєСЂС‹РІР°РµС‚ РµРіРѕ РїРѕ РїСѓС‚Рё РѕС‚РґРµР»СЊРЅС‹Рј
            // РєРѕРЅРЅРµРєС€РЅРѕРј (Р° РЅРµ РёР· РїРµСЂРµРґР°РЅРЅРѕРіРѕ &Connection).
            let conn = open_db(src_str).unwrap();
            init_schema(&conn).unwrap();
            upsert_object_type(
                &conn,
                &make_object_type("backup_chunk_fixture_type", "Note"),
            )
            .unwrap();
            upsert_object(
                &conn,
                &make_object("obj-chunked", "backup_chunk_fixture_type", "Chunked backup"),
            )
            .unwrap();
        }

        let dest = tmp.path().join("ark.db.backup");
        let dest_str = dest.to_str().unwrap();
        backup_to_file_chunked(src_str, dest_str, 4, std::time::Duration::from_millis(0))
            .expect("chunked backup РґРѕР»Р¶РµРЅ РїСЂРѕР№С‚Рё");

        assert!(dest.exists(), "С„Р°Р№Р» backup'Р° РґРѕР»Р¶РµРЅ СЃСѓС‰РµСЃС‚РІРѕРІР°С‚СЊ");
        let backup_conn = open_db(dest_str).unwrap();
        let objects = list_objects(&backup_conn).unwrap();
        assert_eq!(
            objects.len(),
            1,
            "backup РґРѕР»Р¶РµРЅ СЃРѕРґРµСЂР¶Р°С‚СЊ РѕСЂРёРіРёРЅР°Р»СЊРЅС‹Р№ РѕР±СЉРµРєС‚"
        );
        assert_eq!(objects[0].id, "obj-chunked");
        assert_eq!(objects[0].title, "Chunked backup");
    }

    // Р¤Р°Р·Р° B вЂ” С‚РµСЃС‚С‹ РІР»РѕР¶РµРЅРЅРѕСЃС‚Рё (RED РґРѕ Р·Р°РјРµРЅС‹ BEGINв†’SAVEPOINT).
