    fn valid_headers(token: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, format!("Bearer {token}").parse().unwrap());
        headers.insert(
            CLIENT_PID_HEADER,
            std::process::id().to_string().parse().unwrap(),
        );
        headers.insert(API_VERSION_HEADER, API_VERSION.parse().unwrap());
        headers
    }

    #[test]
    fn http_auth_requires_token_pid_and_compatible_version() {
        let token = "a".repeat(64);
        assert!(authenticate(&valid_headers(&token), &token).is_ok());

        let mut missing_pid = valid_headers(&token);
        missing_pid.remove(CLIENT_PID_HEADER);
        assert_eq!(
            authenticate(&missing_pid, &token)
                .expect_err("missing PID must fail")
                .status(),
            StatusCode::FORBIDDEN
        );

        let mut wrong_version = valid_headers(&token);
        wrong_version.insert(API_VERSION_HEADER, "2.0.0".parse().unwrap());
        assert_eq!(
            authenticate(&wrong_version, &token)
                .expect_err("major mismatch must fail")
                .status(),
            StatusCode::UPGRADE_REQUIRED
        );

        assert_eq!(
            authenticate(&valid_headers("b"), &token)
                .expect_err("wrong token must fail")
                .status(),
            StatusCode::UNAUTHORIZED
        );
    }

    fn app_test_grant() -> LaunchGrant {
        LaunchGrant {
            package_id: "com.kosmos.app".into(),
            package_version: "1.0.0".into(),
            manifest_digest: "digest".into(),
            rules: vec![crate::runtime_grants::GrantRule {
                type_id: "com.kosmos.note".into(),
                versions: vec!["1.0.0".into()],
                actions: ["read", "create", "update", "delete", "subscribe", "link"]
                    .into_iter()
                    .map(str::to_owned)
                    .collect(),
                fields_read: vec!["title".into(), "props.description".into()],
                fields_write: vec!["title".into(), "props.description".into()],
                relations_read: vec!["related".into()],
                relations_write: vec!["related".into()],
            }],
            capabilities: vec![],
        }
    }

    #[test]
    fn launch_scoped_dictation_rpc_requires_the_exact_grant_operation() {
        let request = || json!({"operation": "dictation.get_state", "params": {}});
        assert!(parse_app_rpc(request(), &app_test_grant()).is_err());

        let mut grant = app_test_grant();
        grant
            .capabilities
            .push(crate::runtime_grants::ScopedCapability::Dictation {
                operations: vec!["dictation.get_state".into()],
            });
        assert_eq!(
            parse_app_rpc(request(), &grant)
                .expect("granted dictation operation")
                .1,
            "dictation.get_state"
        );
        assert!(parse_app_rpc(
            json!({"operation": "dictation.start_recording", "params": {}}),
            &grant,
        )
        .is_err());
        assert!(parse_app_rpc(
            json!({"operation": "dictation.submit_audio", "params": {}}),
            &grant,
        )
        .is_err());
    }

    fn task_test_grant(version: &str) -> LaunchGrant {
        LaunchGrant {
            package_id: "com.kosmos.app".into(),
            package_version: "1.0.0".into(),
            manifest_digest: "digest".into(),
            rules: vec![crate::runtime_grants::GrantRule {
                type_id: "com.kosmos.task".into(),
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
    async fn launch_scoped_task_upsert_requires_exact_granted_version() {
        let dispatcher = crate::engine_dispatch::EngineDispatcher::new(Arc::new(|_| {
            Box::pin(async { Ok(json!({"ok": true, "data": null})) })
        }));
        let client = DispatchClient::default();
        let explicit = authorize_app_request(
            "upsert_object",
            json!({"object": {"typeId":"task_obj", "typeVersion":"1.0.0", "title":"ok"}}),
            &task_test_grant("=1.0.0"),
            &dispatcher,
            &client,
        )
        .await
        .expect("exact version is authorized");
        assert_eq!(explicit["object"]["typeId"], "com.kosmos.task");
        assert_eq!(explicit["object"]["typeVersion"], "1.0.0");

        assert!(authorize_app_request(
            "upsert_object",
            json!({"object": {"typeId":"task_obj", "title":"missing version"}}),
            &task_test_grant(">=1.0.0"),
            &dispatcher,
            &client,
        )
        .await
        .is_err());
    }

    #[tokio::test]
    async fn launch_scoped_task_upsert_rejects_mismatched_version_without_mutation() {
        let dispatcher = crate::engine_dispatch::EngineDispatcher::new(Arc::new(|_| {
            Box::pin(async { Ok(json!({"ok": true, "data": null})) })
        }));
        let client = DispatchClient::default();
        let params = json!({
            "object": {"typeId":"task_obj", "typeVersion":"2.0.0", "title":"wrong"}
        });
        let original = params.clone();
        assert!(authorize_app_request(
            "upsert_object",
            params.clone(),
            &task_test_grant("=1.0.0"),
            &dispatcher,
            &client,
        )
        .await
        .is_err());
        assert_eq!(params, original);
    }

    #[tokio::test]
    async fn launch_scoped_graph_denies_cross_type_and_ungranted_write() {
        let grant = app_test_grant();
        let dispatcher = crate::engine_dispatch::EngineDispatcher::new(Arc::new(|_| {
            Box::pin(async { Ok(json!({"ok": true, "data": null})) })
        }));
        let client = DispatchClient::default();
        assert!(authorize_app_request(
            "list_objects_by_type",
            json!({"type_id":"note_obj"}),
            &grant,
            &dispatcher,
            &client,
        )
        .await
        .is_ok());
        assert!(authorize_app_request(
            "list_objects_by_type",
            json!({"type_id":"com.kosmos.game"}),
            &grant,
            &dispatcher,
            &client,
        )
        .await
        .is_err());
        assert!(authorize_app_request(
            "upsert_object",
            json!({"object": {
                "id":"n1", "typeId":"com.kosmos.note", "typeVersion":"1.0.0",
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
    async fn launch_scoped_upsert_cannot_turn_create_into_update() {
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
                        "exists":true, "typeId":"com.kosmos.note",
                        "typeVersion":"1.0.0", "revision":"revision-1"
                    }}))
                } else if request.operation.as_str() == "get_object" {
                    Ok(json!({"ok": true, "data": {
                        "id":"n1", "typeId":"com.kosmos.note", "typeVersion":"1.0.0"
                    }}))
                } else {
                    Ok(json!({"ok": true, "data": true}))
                }
            })
        }));
        assert!(authorize_app_request(
            "upsert_object",
            json!({"object": {
                "id":"n1", "typeId":"com.kosmos.note", "typeVersion":"1.0.0",
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
    async fn launch_scoped_write_replaces_untrusted_integrity_fields() {
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
                "id":"n2", "typeId":"com.kosmos.note", "typeVersion":"1.0.0",
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
    async fn launch_scoped_delete_replaces_untrusted_snapshot() {
        let dispatcher = crate::engine_dispatch::EngineDispatcher::new(Arc::new(|request| {
            Box::pin(async move {
                assert_eq!(request.operation.as_str(), "get_object_write_snapshot");
                Ok(json!({"ok": true, "data": {
                    "exists":true, "typeId":"com.kosmos.note",
                    "typeVersion":"1.0.0", "revision":"trusted"
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
    async fn launch_scoped_search_never_returns_mixed_field_snippets() {
        let dispatcher = crate::engine_dispatch::EngineDispatcher::new(Arc::new(|_| {
            Box::pin(async {
                Ok(json!({"ok": true, "data": {
                    "id":"n1", "typeId":"com.kosmos.note", "typeVersion":"1.0.0",
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
        )
        .await;
        assert_eq!(response["data"][0]["text"], "");
    }

    #[test]
    fn launch_scoped_filter_removes_ungranted_fields_and_types() {
        let grant = app_test_grant();
        let mut response = json!({"ok":true,"data":[
            {"id":"n1","typeId":"com.kosmos.note","typeVersion":"1.0.0","title":"title","contentJson":{"secret":true},"propsJson":{"description":"ok","secret":"no"}},
            {"id":"g1","typeId":"com.kosmos.game","typeVersion":"1.0.0","title":"private","propsJson":{}}
        ]});
        filter_object_array(response.get_mut("data").unwrap(), &grant);
        assert_eq!(response["data"].as_array().unwrap().len(), 1);
        let object = &response["data"][0];
        assert_eq!(object["title"], "title");
        assert!(object.get("contentJson").is_none());
        assert_eq!(object["propsJson"]["description"], "ok");
        assert!(object["propsJson"].get("secret").is_none());
    }

    #[test]
    fn launch_scoped_token_is_independent_and_revoked_with_lease() {
        let mut leases = LaunchLeaseRegistry::default();
        let lease = leases
            .create_with_typed_grant(
                AssetGrant {
                    id: "com.kosmos.app".into(),
                    version: "1.0.0".into(),
                    hash: "a".repeat(64),
                },
                app_test_grant(),
            )
            .expect("typed lease");
        let token = lease.launch_token.clone().expect("launch token");
        assert_ne!(token, lease.asset_token);
        assert!(leases.typed_grant(&lease.launch_id, &token).is_some());
        assert!(leases.typed_grant(&lease.launch_id, "wrong").is_none());
        assert!(leases.revoke(&lease.launch_id));
        assert!(leases.typed_grant(&lease.launch_id, &token).is_none());
    }

    #[test]
    fn asset_paths_decode_traversal_and_headers_are_deterministic() {
        assert_eq!(
            percent_decode("dist%2Findex.html").as_deref(),
            Some("dist/index.html")
        );
        assert_eq!(
            percent_decode("%2e%2e%2Fsecret").as_deref(),
            Some("../secret")
        );
        assert!(percent_decode("bad%ZZ").is_none());

        let html = asset_response("index.html", b"<html />".to_vec());
        assert_eq!(
            html.headers().get(CONTENT_TYPE).unwrap(),
            "text/html; charset=utf-8"
        );
        assert_eq!(html.headers().get("cache-control").unwrap(), "no-store");
        assert_eq!(
            html.headers().get("referrer-policy").unwrap(),
            "no-referrer"
        );
        assert!(html.headers().contains_key("content-security-policy"));

        let css = asset_response("styles.css", b"body{}".to_vec());
        assert_eq!(
            css.headers().get(CONTENT_TYPE).unwrap(),
            "text/css; charset=utf-8"
        );
        assert!(!css.headers().contains_key("content-security-policy"));
    }

    #[test]
    fn launch_payload_redacts_store_fields() {
        let package = crate::package_store::InstalledPackage {
            id: "com.kosmos.demo".into(),
            version: "1.0.0".into(),
            hash: "a".repeat(64),
            manifest: crate::package_manifest::VersionedManifest::V1(
                crate::package_manifest::PackageManifest {
                    schema_version: 1,
                    id: "com.kosmos.demo".into(),
                    name: "Demo".into(),
                    version: "1.0.0".into(),
                    kind: crate::package_manifest::PackageKind::App,
                    engine_api: ">=1.0.0".into(),
                    entrypoint: "index.html".into(),
                    publisher: "kosmos".into(),
                    permissions: vec![],
                },
            ),
            enabled: true,
            revoked: false,
            installed_at: 1,
            catalog_sequence: 1,
        };
        let lease = LaunchLeaseRegistry::default()
            .create(AssetGrant {
                id: package.id.clone(),
                version: package.version.clone(),
                hash: package.hash.clone(),
            })
            .expect("lease");
        let body = launch_payload(1234, &lease, &package, LAUNCH_LEASE_TTL).to_string();
        assert!(body.contains(&lease.asset_token));
        assert!(!body.contains(&package.hash));
        for forbidden in ["bearer", "signature", "public_key", "\\\\blobs\\\\"] {
            assert!(!body.contains(forbidden), "leaked {forbidden}");
        }
    }

    #[test]
    fn launch_leases_are_unique_and_independently_revocable() {
        let mut leases = LaunchLeaseRegistry::default();
        let first = leases
            .create(AssetGrant {
                id: "com.kosmos.demo".into(),
                version: "1.0.0".into(),
                hash: "a".repeat(64),
            })
            .expect("first lease");
        let second = leases
            .create(AssetGrant {
                id: "com.kosmos.demo".into(),
                version: "1.0.0".into(),
                hash: "a".repeat(64),
            })
            .expect("second lease");
        assert_ne!(first.launch_id, second.launch_id);
        assert_ne!(first.asset_token, second.asset_token);
        assert!(leases.asset(&first.asset_token).is_some());
        assert!(leases.revoke(&first.launch_id));
        assert!(leases.asset(&first.asset_token).is_none());
        assert!(leases.asset(&second.asset_token).is_some());
        assert!(!leases.revoke("not-a-launch-id"));
    }

    #[test]
    fn lease_registry_purges_expired_before_capacity_and_never_evicts_live_leases() {
        let mut leases = LaunchLeaseRegistry::with_limits(Duration::from_secs(1), 2);
        let first = leases
            .try_create_at(
                AssetGrant {
                    id: "com.kosmos.demo".into(),
                    version: "1.0.0".into(),
                    hash: "a".repeat(64),
                },
                Instant::now(),
            )
            .expect("first live lease");
        let second = leases
            .try_create_at(
                AssetGrant {
                    id: "com.kosmos.demo".into(),
                    version: "1.0.0".into(),
                    hash: "b".repeat(64),
                },
                Instant::now(),
            )
            .expect("second live lease");
        assert!(matches!(
            leases.try_create_at(
                AssetGrant {
                    id: "com.kosmos.demo".into(),
                    version: "1.0.0".into(),
                    hash: "c".repeat(64),
                },
                Instant::now(),
            ),
            Err(LeaseCapacityError)
        ));
        assert!(leases.asset(&first.asset_token).is_some());
        assert!(leases.asset(&second.asset_token).is_some());
        leases.purge_expired_at(Instant::now() + Duration::from_secs(2));
        assert_eq!(leases.len(), 0);
    }

    #[test]
    fn resolve_payload_is_manifest_only_and_never_contains_launch_fields() {
        let package = crate::package_store::InstalledPackage {
            id: "com.kosmos.demo".into(),
            version: "1.0.0".into(),
            hash: "a".repeat(64),
            manifest: crate::package_manifest::VersionedManifest::V1(
                crate::package_manifest::PackageManifest {
                    schema_version: 1,
                    id: "com.kosmos.demo".into(),
                    name: "Demo".into(),
                    version: "1.0.0".into(),
                    kind: crate::package_manifest::PackageKind::App,
                    engine_api: ">=1.0.0".into(),
                    entrypoint: "index.html".into(),
                    publisher: "kosmos".into(),
                    permissions: vec![],
                },
            ),
            enabled: true,
            revoked: false,
            installed_at: 1,
            catalog_sequence: 1,
        };
        let body = resolve_payload(&package).to_string();
        assert!(body.contains("\"id\":\"com.kosmos.demo\""));
        for launch_only in [
            "launch_url",
            "launch_id",
            "asset_token",
            "expires_at",
            "ttl_seconds",
        ] {
            assert!(!body.contains(launch_only), "resolve leaked {launch_only}");
        }
    }

    #[test]
    fn repeated_resolve_payloads_never_evict_an_active_lease() {
        let mut leases = LaunchLeaseRegistry::default();
        let lease = leases
            .create(AssetGrant {
                id: "com.kosmos.demo".into(),
                version: "1.0.0".into(),
                hash: "a".repeat(64),
            })
            .expect("lease");
        let package = crate::package_store::InstalledPackage {
            id: "com.kosmos.demo".into(),
            version: "1.0.0".into(),
            hash: "a".repeat(64),
            manifest: crate::package_manifest::VersionedManifest::V1(
                crate::package_manifest::PackageManifest {
                    schema_version: 1,
                    id: "com.kosmos.demo".into(),
                    name: "Demo".into(),
                    version: "1.0.0".into(),
                    kind: crate::package_manifest::PackageKind::App,
                    engine_api: ">=1.0.0".into(),
                    entrypoint: "index.html".into(),
                    publisher: "kosmos".into(),
                    permissions: vec![],
                },
            ),
            enabled: true,
            revoked: false,
            installed_at: 1,
            catalog_sequence: 1,
        };
        for _ in 0..10_000 {
            assert!(resolve_payload(&package).get("data").is_some());
        }
        assert!(leases.asset(&lease.asset_token).is_some());
    }

    #[tokio::test]
    async fn live_http_rejects_unsupported_malformed_and_oversized_requests() {
        let dir = tempfile::tempdir().unwrap();
        let usage = Arc::new(ProtocolUsageStore::open(dir.path()).unwrap());
        let token = "a".repeat(64);
        let server = EngineApiServer::bind(
            token.clone(),
            9,
            usage,
            "00000000-0000-4000-8000-000000000001".into(),
            Arc::new(PackageService::open(dir.path()).unwrap()),
            test_dispatcher(),
        )
        .await
        .unwrap();
        let port = server.port();
        let task = tokio::spawn(server.run());

        let unauthorized =
            raw_http(port, "GET /v1/health HTTP/1.1\r\nConnection: close\r\n\r\n").await;
        assert!(unauthorized.starts_with("HTTP/1.1 401"));

        let unsupported = raw_http(port, &request(&token, "POST", "/v1/health", "")).await;
        assert!(unsupported.starts_with("HTTP/1.1 405"));
        let unknown = raw_http(port, &request(&token, "GET", "/v1/unknown", "")).await;
        assert!(unknown.starts_with("HTTP/1.1 404"));

        let malformed = raw_http(port, &request(&token, "POST", "/v1/rpc", "{")).await;
        assert!(malformed.starts_with("HTTP/1.1 400"));

        let missing_operation = raw_http(port, &request(&token, "POST", "/v1/rpc", "{}")).await;
        assert!(missing_operation.starts_with("HTTP/1.1 400"));

        let oversized_body = format!(
            r#"{{"operation":"noop","padding":"{}"}}"#,
            "x".repeat(MAX_HTTP_BODY_BYTES)
        );
        let oversized = raw_http(port, &request(&token, "POST", "/v1/rpc", &oversized_body)).await;
        assert!(oversized.starts_with("HTTP/1.1 413"));

        task.abort();
    }

    #[tokio::test]
    async fn loopback_http_launch_lease_lifecycle_uses_signed_installed_package() {
        let dir = tempfile::tempdir().expect("tempdir");
        let token = "a".repeat(64);
        let (service, _, _) = crate::package_service::tests::enabled_app_service(dir.path());
        let server = EngineApiServer::bind(
            token.clone(),
            9,
            Arc::new(ProtocolUsageStore::open(dir.path()).expect("usage")),
            "00000000-0000-4000-8000-000000000001".into(),
            Arc::new(service),
            test_dispatcher(),
        )
        .await
        .expect("server");
        let port = server.port();
        let task = tokio::spawn(server.run());

        // Cover every new route against the real Engine v1 auth boundary;
        // no route is allowed to infer authorization from a launch-only test.
        for (method, path, body) in [
            ("POST", "/v1/apps/resolve", r#"{"id":"com.kosmos.demo"}"#),
            ("POST", "/v1/apps/launch", r#"{"id":"com.kosmos.demo"}"#),
            (
                "DELETE",
                "/v1/apps/launch/00000000-0000-4000-8000-000000000099",
                "",
            ),
        ] {
            let cases = [
                (
                    "missing bearer",
                    format!("{method} {path} HTTP/1.1\r\nConnection: close\r\n\r\n"),
                    "HTTP/1.1 401",
                ),
                (
                    "wrong bearer",
                    request_with_headers(
                        "b",
                        method,
                        path,
                        body,
                        &std::process::id().to_string(),
                        API_VERSION,
                    ),
                    "HTTP/1.1 401",
                ),
                (
                    "dead pid",
                    request_with_headers(&token, method, path, body, "2147483647", API_VERSION),
                    "HTTP/1.1 403",
                ),
                (
                    "incompatible API",
                    request_with_headers(
                        &token,
                        method,
                        path,
                        body,
                        &std::process::id().to_string(),
                        "2.0.0",
                    ),
                    "HTTP/1.1 426",
                ),
            ];
            for (name, invalid, expected) in cases {
                let response = raw_http(port, &invalid).await;
                assert!(
                    response.starts_with(expected),
                    "{name} for {method} {path}: expected {expected}, got {response}"
                );
            }
        }

        let first = response_json(
            &raw_http(
                port,
                &request(
                    &token,
                    "POST",
                    "/v1/apps/launch",
                    r#"{"id":"com.kosmos.demo"}"#,
                ),
            )
            .await,
        );
        let first_data = &first["data"];
        let first_id = first_data["launch_id"]
            .as_str()
            .expect("launch id")
            .to_owned();
        let first_url = first_data["launch_url"]
            .as_str()
            .expect("asset url")
            .to_owned();
        let first_path = first_url
            .split_once(&format!(":{port}"))
            .expect("asset path in loopback URL")
            .1;
        assert!(raw_http(
            port,
            &format!("GET {first_path} HTTP/1.1\r\nConnection: close\r\n\r\n")
        )
        .await
        .starts_with("HTTP/1.1 200"));

        for _ in 0..10_000 {
            let resolved = response_json(
                &raw_http(
                    port,
                    &request(
                        &token,
                        "POST",
                        "/v1/apps/resolve",
                        r#"{"id":"com.kosmos.demo"}"#,
                    ),
                )
                .await,
            );
            assert!(resolved["data"].get("launch_id").is_none());
        }
        assert!(raw_http(
            port,
            &format!("GET {first_path} HTTP/1.1\r\nConnection: close\r\n\r\n")
        )
        .await
        .starts_with("HTTP/1.1 200"));

        let second = response_json(
            &raw_http(
                port,
                &request(
                    &token,
                    "POST",
                    "/v1/apps/launch",
                    r#"{"id":"com.kosmos.demo"}"#,
                ),
            )
            .await,
        );
        let second_id = second["data"]["launch_id"]
            .as_str()
            .expect("second launch id");
        let malformed = raw_http(
            port,
            &request(&token, "DELETE", "/v1/apps/launch/not-a-uuid", ""),
        )
        .await;
        assert!(malformed.starts_with("HTTP/1.1 404"));
        let unknown = raw_http(
            port,
            &request(
                &token,
                "DELETE",
                "/v1/apps/launch/00000000-0000-4000-8000-000000000099",
                "",
            ),
        )
        .await;
        assert!(unknown.starts_with("HTTP/1.1 404"));
        let second_path = second["data"]["launch_url"]
            .as_str()
            .expect("second url")
            .split_once(&format!(":{port}"))
            .expect("asset path in loopback URL")
            .1;
        assert!(raw_http(
            port,
            &format!("GET {second_path} HTTP/1.1\r\nConnection: close\r\n\r\n")
        )
        .await
        .starts_with("HTTP/1.1 200"));
        assert_ne!(first_id, second_id);

        assert!(raw_http(
            port,
            &request(&token, "DELETE", &format!("/v1/apps/launch/{first_id}"), "")
        )
        .await
        .starts_with("HTTP/1.1 200"));
        assert!(raw_http(
            port,
            &format!("GET {first_path} HTTP/1.1\r\nConnection: close\r\n\r\n")
        )
        .await
        .starts_with("HTTP/1.1 404"));
        task.abort();
    }

    #[tokio::test]
    async fn lifecycle_cleanup_purges_an_idle_lease_without_a_registry_request() {
        let dir = tempfile::tempdir().expect("tempdir");
        let token = "a".repeat(64);
        let (service, _, _) = crate::package_service::tests::enabled_app_service(dir.path());
        let server = EngineApiServer::bind_with_test_limits(
            token.clone(),
            9,
            Arc::new(ProtocolUsageStore::open(dir.path()).expect("usage")),
            "00000000-0000-4000-8000-000000000001".into(),
            Arc::new(service),
            Duration::from_secs(300),
            2,
            Duration::from_millis(5),
        )
        .await
        .expect("server");
        let port = server.port();
        let leases = Arc::clone(&server.launch_leases);
        let task = tokio::spawn(server.run());
        let launched = response_json(
            &raw_http(
                port,
                &request(
                    &token,
                    "POST",
                    "/v1/apps/launch",
                    r#"{"id":"com.kosmos.demo"}"#,
                ),
            )
            .await,
        );
        let path = launched["data"]["launch_url"]
            .as_str()
            .expect("url")
            .split_once(&format!(":{port}"))
            .expect("loopback path")
            .1
            .to_owned();
        let launch_id = launched["data"]["launch_id"].as_str().expect("launch id");

        expire_launch_for_test(&leases, launch_id);
        wait_for_lease_count(&leases, 0).await;
        assert!(raw_http(
            port,
            &format!("GET {path} HTTP/1.1\r\nConnection: close\r\n\r\n")
        )
        .await
        .starts_with("HTTP/1.1 404"));
        task.abort();
    }

    #[tokio::test]
    async fn http_capacity_preserves_live_assets_and_accepts_a_launch_after_lifecycle_purge() {
        let dir = tempfile::tempdir().expect("tempdir");
        let token = "a".repeat(64);
        let (service, _, _) = crate::package_service::tests::enabled_app_service(dir.path());
        let server = EngineApiServer::bind_with_test_limits(
            token.clone(),
            9,
            Arc::new(ProtocolUsageStore::open(dir.path()).expect("usage")),
            "00000000-0000-4000-8000-000000000001".into(),
            Arc::new(service),
            Duration::from_secs(300),
            2,
            Duration::from_millis(5),
        )
        .await
        .expect("server");
        let port = server.port();
        let leases = Arc::clone(&server.launch_leases);
        let task = tokio::spawn(server.run());
        let launch = || {
            request(
                &token,
                "POST",
                "/v1/apps/launch",
                r#"{"id":"com.kosmos.demo"}"#,
            )
        };
        let first = response_json(&raw_http(port, &launch()).await);
        let first_path = first["data"]["launch_url"]
            .as_str()
            .expect("first url")
            .split_once(&format!(":{port}"))
            .expect("first path")
            .1
            .to_owned();
        let first_id = first["data"]["launch_id"]
            .as_str()
            .expect("first launch id")
            .to_owned();
        let second = response_json(&raw_http(port, &launch()).await);
        let second_path = second["data"]["launch_url"]
            .as_str()
            .expect("second url")
            .split_once(&format!(":{port}"))
            .expect("second path")
            .1
            .to_owned();
        assert!(raw_http(port, &launch()).await.starts_with("HTTP/1.1 429"));
        for path in [&first_path, &second_path] {
            assert!(raw_http(
                port,
                &format!("GET {path} HTTP/1.1\r\nConnection: close\r\n\r\n")
            )
            .await
            .starts_with("HTTP/1.1 200"));
        }

        expire_launch_for_test(&leases, &first_id);
        wait_for_lease_count(&leases, 1).await;
        assert!(raw_http(
            port,
            &format!("GET {first_path} HTTP/1.1\r\nConnection: close\r\n\r\n")
        )
        .await
        .starts_with("HTTP/1.1 404"));
        assert!(raw_http(
            port,
            &format!("GET {second_path} HTTP/1.1\r\nConnection: close\r\n\r\n")
        )
        .await
        .starts_with("HTTP/1.1 200"));
        assert!(raw_http(port, &launch()).await.starts_with("HTTP/1.1 200"));
        assert!(raw_http(
            port,
            &format!("GET {second_path} HTTP/1.1\r\nConnection: close\r\n\r\n")
        )
        .await
        .starts_with("HTTP/1.1 200"));
        task.abort();
    }

    fn expire_launch_for_test(leases: &Arc<Mutex<LaunchLeaseRegistry>>, launch_id: &str) {
        let mut leases = leases
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        leases
            .leases
            .get_mut(launch_id)
            .expect("launch lease")
            .expires_at = Instant::now() - Duration::from_secs(1);
    }

    async fn wait_for_lease_count(leases: &Arc<Mutex<LaunchLeaseRegistry>>, expected: usize) {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let count = leases
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .len();
                if count == expected {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("lifecycle cleanup deadline");
    }
