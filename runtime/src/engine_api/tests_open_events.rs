    // KOS-299: launch-scoped ARK events over SSE — grant-filtered, ending
    // on revoke. Shares the fixture/helpers from tests_open.rs (same `mod tests`).


    /// Allowed types stream through, ungranted types are dropped, and a
    /// revoked lease's stream ends.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn launch_events_stream_filters_by_grant_and_ends_on_revoke() {
        let dir = tempfile::tempdir().expect("tempdir");
        let token = "a".repeat(64);
        let service = Arc::new(
            crate::package_service::tests::enabled_note_write_app_service(dir.path()),
        );
        let fixture = open_engine(dir, &token, service).await;
        let port = fixture.port;
        let origin = format!("http://127.0.0.1:{port}");

        let opened = rpc_open(port, &token, "com.kosmos.demo").await;
        let (_, launch_id, code) =
            launch_bootstrap_parts(opened["data"]["launch_url"].as_str().unwrap(), port);
        let session = response_json(&bootstrap(port, &launch_id, &code, &origin).await);
        let launch_token = session["data"]["broker_token"].as_str().unwrap().to_string();

        let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .expect("connect");
        stream
            .write_all(
                format!(
                    "GET /v1/apps/launch/{launch_id}/events HTTP/1.1\r\n\
                     Host: 127.0.0.1:{port}\r\n\
                     X-Kosmos-Launch-Token: {launch_token}\r\n\
                     Connection: close\r\n\r\n"
                )
                .as_bytes(),
            )
            .await
            .expect("write events request");

        // Read response headers first.
        let mut captured = String::new();
        tokio::time::timeout(Duration::from_secs(5), async {
            let mut byte = [0u8; 1];
            while !captured.ends_with("\r\n\r\n") {
                stream.read_exact(&mut byte).await.expect("header byte");
                captured.push(byte[0] as char);
            }
        })
        .await
        .expect("events response headers");
        assert!(captured.starts_with("HTTP/1.1 200"), "{captured}");
        assert!(captured.contains("text/event-stream"), "{captured}");

        let mut read_buf = vec![0u8; 4096];

        // Filtered type must not be delivered; the granted one must.
        fixture.ark.emit_event(json!({
            "event": "object_upserted",
            "id": "o-denied",
            "type_id": "com.secret.type",
        }));
        tokio::time::sleep(Duration::from_millis(50)).await;
        fixture.ark.emit_event(json!({
            "event": "object_upserted",
            "id": "o-allowed",
            "type_id": "com.kosmos.note",
        }));
        let window = drain_until(&mut stream, "o-allowed", &mut read_buf).await;
        assert!(window.contains("o-allowed"), "{window}");
        assert!(!window.contains("o-denied"), "{window}");
        assert!(!window.contains("com.secret.type"), "{window}");

        // Revocation ends the stream — a dead session must not keep
        // receiving events.
        let body = format!(r#"{{"token":"{launch_token}"}}"#);
        let revoked = raw_http(
            port,
            &format!(
                "POST /v1/apps/launch/{launch_id}/revoke HTTP/1.1\r\n\
                 Host: 127.0.0.1:{port}\r\n\
                 Content-Length: {}\r\n\
                 Connection: close\r\n\r\n{body}",
                body.len()
            ),
        )
        .await;
        assert!(revoked.starts_with("HTTP/1.1 200"), "{revoked}");
        fixture.ark.emit_event(json!({
            "event": "object_upserted",
            "id": "o-after-revoke",
            "type_id": "com.kosmos.note",
        }));
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let n = stream.read(&mut read_buf).await.expect("eof read");
                if n == 0 {
                    return;
                }
            }
        })
        .await
        .expect("revoked event stream must end");

        fixture.shutdown().await;
    }

