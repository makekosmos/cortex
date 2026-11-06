use super::*;

pub(super) fn task_test_grant(version: &str) -> LaunchGrant {
    LaunchGrant {
        package_id: "com.kosmos.app".into(),
        package_version: "1.0.0".into(),
        manifest_digest: "digest".into(),
        rules: vec![crate::runtime_grants::GrantRule {
            type_id: "core.task".into(),
            versions: vec![version.into()],
            actions: ["create"].into_iter().map(str::to_owned).collect(),
            fields_read: vec![],
            fields_write: vec!["title".into()],
            relations_read: vec![],
            relations_write: vec![],
        }],
        capabilities: vec![],
    }
}

#[tokio::test]
pub(super) async fn launch_scoped_task_upsert_requires_exact_granted_version() {
    let dispatcher = crate::engine_dispatch::EngineDispatcher::new(Arc::new(|_| {
        Box::pin(async { Ok(json!({"ok": true, "data": null})) })
    }));
    let client = DispatchClient::default();
    let explicit = authorize_app_request(
        "upsert_object",
        json!({"object": {"typeId":"core.task", "typeVersion":"0.1.0", "title":"ok"}}),
        &task_test_grant("=0.1.0"),
        &dispatcher,
        &client,
    )
    .await
    .expect("exact version is authorized");
    assert_eq!(explicit["object"]["typeId"], "core.task");
    assert_eq!(explicit["object"]["typeVersion"], "0.1.0");

    assert!(authorize_app_request(
        "upsert_object",
        json!({"object": {"typeId":"core.task", "title":"missing version"}}),
        &task_test_grant(">=0.1.0"),
        &dispatcher,
        &client,
    )
    .await
    .is_err());
}

#[tokio::test]
pub(super) async fn launch_scoped_task_upsert_rejects_mismatched_version_without_mutation() {
    let dispatcher = crate::engine_dispatch::EngineDispatcher::new(Arc::new(|_| {
        Box::pin(async { Ok(json!({"ok": true, "data": null})) })
    }));
    let client = DispatchClient::default();
    let params = json!({
        "object": {"typeId":"core.task", "typeVersion":"0.2.0", "title":"wrong"}
    });
    let original = params.clone();
    assert!(authorize_app_request(
        "upsert_object",
        params.clone(),
        &task_test_grant("=0.1.0"),
        &dispatcher,
        &client,
    )
    .await
    .is_err());
    assert_eq!(params, original);
}

#[tokio::test]
pub(super) async fn launch_scoped_graph_denies_cross_type_and_ungranted_write() {
    let grant = app_test_grant();
    let dispatcher = crate::engine_dispatch::EngineDispatcher::new(Arc::new(|_| {
        Box::pin(async { Ok(json!({"ok": true, "data": null})) })
    }));
    let client = DispatchClient::default();
    assert!(authorize_app_request(
        "list_objects_by_type",
        json!({"type_id":"core.note"}),
        &grant,
        &dispatcher,
        &client,
    )
    .await
    .is_ok());
    assert!(authorize_app_request(
        "list_objects_by_type",
        json!({"type_id":"core.game"}),
        &grant,
        &dispatcher,
        &client,
    )
    .await
    .is_err());
    assert!(authorize_app_request(
        "upsert_object",
        json!({"object": {
            "id":"n1", "typeId":"core.note", "typeVersion":"0.1.0",
            "title":"ok", "propsJson":{"secret":"no"}
        }}),
        &grant,
        &dispatcher,
        &client,
    )
    .await
    .is_err());
}

#[tokio::test]
pub(super) async fn launch_scoped_upsert_cannot_turn_create_into_update() {
    let mut grant = app_test_grant();
    grant
        .rules
        .first_mut()
        .expect("rule")
        .actions
        .remove("update");
    let dispatcher = crate::engine_dispatch::EngineDispatcher::new(Arc::new(|request| {
        Box::pin(async move {
            if request.operation.as_str() == "get_object_write_snapshot" {
                Ok(json!({"ok": true, "data": {
                    "exists":true, "typeId":"core.note",
                    "typeVersion":"0.1.0", "revision":"revision-1"
                }}))
            } else if request.operation.as_str() == "get_object" {
                Ok(json!({"ok": true, "data": {
                    "id":"n1", "typeId":"core.note", "typeVersion":"0.1.0"
                }}))
            } else {
                Ok(json!({"ok": true, "data": true}))
            }
        })
    }));
    assert!(authorize_app_request(
        "upsert_object",
        json!({"object": {
            "id":"n1", "typeId":"core.note", "typeVersion":"0.1.0",
            "title":"overwrite"
        }}),
        &grant,
        &dispatcher,
        &DispatchClient::default(),
    )
    .await
    .is_err());
}

#[tokio::test]
pub(super) async fn launch_scoped_write_replaces_untrusted_integrity_fields() {
    let dispatcher = crate::engine_dispatch::EngineDispatcher::new(Arc::new(|request| {
        Box::pin(async move {
            if request.operation.as_str() == "get_object_write_snapshot" {
                Ok(json!({"ok": true, "data": {
                    "exists":false, "typeId":null, "typeVersion":null, "revision":null
                }}))
            } else {
                Ok(json!({"ok": true, "data": null}))
            }
        })
    }));
    let params = authorize_app_request(
        "upsert_object",
        json!({"expectedSnapshot":{"exists":true,"revision":"forged"}, "object": {
            "id":"n2", "typeId":"core.note", "typeVersion":"0.1.0",
            "title":"ok", "propsJson":{"description":"allowed"},
            "createdAt":"forged", "deletedAt":"forged", "unexpected":"forged"
        }}),
        &app_test_grant(),
        &dispatcher,
        &DispatchClient::default(),
    )
    .await
    .expect("authorized create");
    let object = params["object"].as_object().expect("object");
    for field in ["createdAt", "updatedAt"] {
        let timestamp = object[field].as_str().expect("server timestamp");
        assert_ne!(timestamp, "forged");
        chrono::DateTime::parse_from_rfc3339(timestamp).expect("RFC 3339 timestamp");
    }
    assert!(object["deletedAt"].is_null());
    assert!(!object.contains_key("unexpected"));
    assert_eq!(
        params["expectedSnapshot"],
        json!({"exists":false,"typeId":null,"typeVersion":null,"revision":null})
    );
}

#[tokio::test]
pub(super) async fn launch_scoped_delete_replaces_untrusted_snapshot() {
    let dispatcher = crate::engine_dispatch::EngineDispatcher::new(Arc::new(|request| {
        Box::pin(async move {
            assert_eq!(request.operation.as_str(), "get_object_write_snapshot");
            Ok(json!({"ok": true, "data": {
                "exists":true, "typeId":"core.note",
                "typeVersion":"0.1.0", "revision":"trusted"
            }}))
        })
    }));
    let params = authorize_app_request(
        "delete_object",
        json!({"id":"n2", "expectedSnapshot":{"revision":"forged"}}),
        &app_test_grant(),
        &dispatcher,
        &DispatchClient::default(),
    )
    .await
    .expect("authorized delete");
    assert_eq!(params["expectedSnapshot"]["revision"], "trusted");
}

#[tokio::test]
pub(super) async fn launch_scoped_search_never_returns_mixed_field_snippets() {
    let dispatcher = crate::engine_dispatch::EngineDispatcher::new(Arc::new(|_| {
        Box::pin(async {
            Ok(json!({"ok": true, "data": {
                "id":"n1", "typeId":"core.note", "typeVersion":"0.1.0",
                "title":"allowed", "contentJson":"private"
            }}))
        })
    }));
    let response = filter_app_response(
        "search_objects",
        json!({"ok":true,"data":[{"entryId":"n1","text":"private snippet"}]}),
        &app_test_grant(),
        &dispatcher,
        &DispatchClient::default(),
        None,
    )
    .await;
    assert_eq!(response["data"][0]["text"], "");
}

#[tokio::test]
pub(super) async fn launch_scoped_dictation_responses_expose_only_renderer_fields() {
    let dispatcher = crate::engine_dispatch::EngineDispatcher::new(Arc::new(|_| {
        Box::pin(async { Ok(json!(null)) })
    }));
    let client = DispatchClient::default();

    let config = filter_app_response(
        "dictation.get_config",
        json!({"ok":true,"data":{
            "config": {
                "hotkey":"Ctrl+Shift+;", "language":"ru", "injectMode":"auto_paste",
                "provider":"local", "model":"whisper", "localModelId":"small",
                "providerEnabled":true, "httpProxy":"http://proxy.invalid",
                "transcriptionPrompt":"secret prompt", "localModelPath":"C:\\secret",
                "localCommandPath":"C:\\secret\\whisper.exe", "microphoneDeviceId":"device",
                "networkProfile":{"kind":"custom_doh","url":"https://dns.invalid"}
            },
            "hasApiKey":true, "apiKey":"secret", "error":"internal",
            "activeUuid":"00000000-0000-4000-8000-000000000001"
        }}),
        &app_test_grant(),
        &dispatcher,
        &client,
        None,
    )
    .await;
    assert_eq!(
        config,
        json!({"ok":true,"data":{
            "config": {
                "hotkey":"Ctrl+Shift+;", "language":"ru", "injectMode":"auto_paste",
                "provider":"local", "model":"whisper", "localModelId":"small",
                "providerEnabled":true
            },
            "hasApiKey":true
        }})
    );

    let updated = filter_app_response(
        "dictation.update_config",
        json!({"ok":true,"data":{"config":{
            "language":"en", "provider":"groq", "providerEnabled":true,
            "httpProxy":"http://proxy.invalid", "localModelPath":"C:\\secret"
        }}}),
        &app_test_grant(),
        &dispatcher,
        &client,
        None,
    )
    .await;
    assert_eq!(
        updated,
        json!({"ok":true,"data":{"config":{
            "language":"en", "provider":"groq", "providerEnabled":true
        }}})
    );

    let state = filter_app_response(
        "dictation.get_state",
        json!({"ok":true,"data":{
            "state":"error", "microphonePermission":"denied", "hasApiKey":true,
            "lastError":"C:\\private\\trace", "activeUuid":"uuid", "attempts":3,
            "canRetry":true, "config":{"httpProxy":"secret"}
        }}),
        &app_test_grant(),
        &dispatcher,
        &client,
        None,
    )
    .await;
    assert_eq!(
        state,
        json!({"ok":true,"data":{"state":"error","microphonePermission":"denied"}})
    );

    let models = filter_app_response(
        "dictation.list_local_models",
        json!({"ok":true,"data":{
            "commandInstalled":true, "models":[
                {"id":"small", "name":"Whisper Small", "transcriptionSupported":true,
                 "directory":false, "downloaded":true, "path":"C:\\models\\small",
                 "url":"https://models.invalid/small", "filename":"small.bin",
                 "description":"private", "selected":true},
                {"id":42, "name":"drop this hostile entry", "path":"C:\\secret"}
            ],
            "modelsDir":"C:\\models", "commandPath":"C:\\tools\\whisper.exe"
        }}),
        &app_test_grant(),
        &dispatcher,
        &client,
        None,
    )
    .await;
    assert_eq!(
        models,
        json!({"ok":true,"data":{
            "commandInstalled":true,
            "models":[{"id":"small","name":"Whisper Small",
                "transcriptionSupported":true,"directory":false,"downloaded":true}]
        }})
    );
}

#[tokio::test]
pub(super) async fn launch_scoped_dictation_filter_redacts_error_responses() {
    let dispatcher = crate::engine_dispatch::EngineDispatcher::new(Arc::new(|_| {
        Box::pin(async { Ok(json!(null)) })
    }));
    let response = json!({
        "ok": false,
        "error": {"message":"internal", "path":"C:\\private", "uuid":"secret"},
        "data": {"config": {"httpProxy":"must remain unchanged"}}
    });
    assert_eq!(
        filter_app_response(
            "dictation.get_config",
            response.clone(),
            &app_test_grant(),
            &dispatcher,
            &DispatchClient::default(),
            None,
        )
        .await,
        json!({"ok":false,"error":"unavailable"})
    );
}

#[test]
pub(super) fn launch_scoped_filter_removes_ungranted_fields_and_types() {
    let grant = app_test_grant();
    let mut response = json!({"ok":true,"data":[
        {
            "id":"n1",
            "typeId":"core.note",
            "typeVersion":"0.1.0",
            "title":"title",
            "contentJson":{"secret":true},
            "propsJson":{"description":"ok","secret":"no"},
        },
        {
            "id":"g1",
            "typeId":"core.game",
            "typeVersion":"0.1.0",
            "title":"private",
            "propsJson":{},
        }
    ]});
    filter_object_array(response.get_mut("data").unwrap(), &grant);
    assert_eq!(response["data"].as_array().unwrap().len(), 1);
    let object = &response["data"][0];
    assert_eq!(object["title"], "title");
    assert!(object.get("contentJson").is_none());
    assert_eq!(object["propsJson"]["description"], "ok");
    assert!(object["propsJson"].get("secret").is_none());
}
