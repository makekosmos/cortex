    #[tokio::test]
    async fn storage_backend_versioned_object_matrix_holds_unknown_payload_and_replays_exactly() {
        let backend = make_backend();
        let conn = backend.conn.clone();
        {
            let guard = conn.lock().unwrap();
            upsert_object_type(&guard, &make_object_type("remote-type", "Remote")).unwrap();
        }
        let mut old = sync_object("remote-old", "remote-type", "old-wire");
        old.data.remove("typeVersion");
        backend.apply_entity(&old).await.unwrap();
        assert_eq!(
            get_object(&conn.lock().unwrap(), "remote-old")
                .unwrap()
                .unwrap()
                .type_version,
            type_registry::LEGACY_VERSION
        );
        let mut unknown = sync_object("remote-unknown", "remote-type", "unknown-wire");
        unknown.data.insert("typeVersion".into(), json!("9.9.9"));
        unknown
            .data
            .insert("futurePayload".into(), json!({"keep":true}));
        backend.apply_entity(&unknown).await.unwrap();
        let guard = conn.lock().unwrap();
        assert!(get_object(&guard, "remote-unknown").unwrap().is_none());
        let payload: String = guard
            .query_row(
                "SELECT payload FROM sync_pending_objects WHERE id='remote-unknown'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(payload.contains("futurePayload"));
        drop(guard);
        let replayed = {
            let guard = conn.lock().unwrap();
            replay_pending_for_type(&guard, "remote-type", type_registry::LEGACY_VERSION).unwrap()
        };
        assert_eq!(replayed, 0);
        assert!(get_object(&conn.lock().unwrap(), "remote-unknown")
            .unwrap()
            .is_none());
    }
