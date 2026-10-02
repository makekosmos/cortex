    // Round-2 hardening: the release grace (a closed tab ends within
    // RELEASE_GRACE; a reload renews inside it and survives), per-package
    // bootstrap origins, and the byte-exact `<head>` injection.

    /// The `<head>` scan must be byte-exact: invalid UTF-8 before the tag
    /// must not shift the insertion point, `<HEAD>` matches, `<header>`
    /// does not, and headless markup gets the tag prepended.
    #[test]
    fn inject_host_shim_scans_raw_bytes_for_the_head_tag() {
        let tag = r#"<script src="__kosmos_host_shim.js"></script>"#;

        // Invalid UTF-8 before <head> — offsets must stay byte-true.
        let injected = super::inject_host_shim("index.html", b"\xff\xfe<html><head><body>x");
        let at = injected
            .windows(tag.len())
            .position(|w| w == tag.as_bytes())
            .expect("tag injected");
        assert_eq!(&injected[..at], b"\xff\xfe<html><head>");

        // Uppercase HEAD with attributes.
        let injected = super::inject_host_shim("index.html", b"<HEAD lang=\"en\"><body>");
        assert!(injected.starts_with(b"<HEAD lang=\"en\"><script"));

        // <header> is not <head>.
        let injected = super::inject_host_shim("index.html", b"<header><body>");
        assert!(injected.starts_with(tag.as_bytes()));
        assert!(injected.ends_with(b"<header><body>"));

        // No head at all → prepend.
        let injected = super::inject_host_shim("index.html", b"<html><body>");
        assert!(injected.starts_with(tag.as_bytes()));

        // Non-HTML assets are untouched.
        let js = b"var head = 1;".to_vec();
        assert_eq!(super::inject_host_shim("app.js", &js), js);
    }

    /// `pagehide` releases rather than revokes: inside the grace window the
    /// lease still serves and a renew (what the reloaded shim does first)
    /// cancels the release; past it, the lease is purged.
    #[tokio::test]
    async fn release_grace_lets_a_reload_renew_then_purges_a_real_close() {
        let dir = tempfile::tempdir().expect("tempdir");
        let token = "a".repeat(64);
        let service =
            Arc::new(crate::package_service::tests::enabled_note_write_app_service(dir.path()));
        let fixture = open_engine(dir, &token, service).await;
        let port = fixture.port;

        let opened = rpc_open(port, &token, "com.kosmos.demo").await;
        let (_, launch_id, code) =
            launch_bootstrap_parts(opened["data"]["launch_url"].as_str().unwrap(), port);
        let session =
            response_json(&bootstrap(port, &launch_id, &code, &package_origin(port)).await);
        let launch_token = session["data"]["broker_token"].as_str().unwrap();

        let release = raw_http(
            port,
            &format!(
                "POST /v1/apps/launch/{launch_id}/release HTTP/1.1\r\n\
                 Host: 127.0.0.1:{port}\r\n\
                 Content-Length: {}\r\n\
                 Connection: close\r\n\r\n{{\"token\":\"{launch_token}\"}}",
                format!(r#"{{"token":"{launch_token}"}}"#).len()
            ),
        )
        .await;
        assert!(release.starts_with("HTTP/1.1 200"), "{release}");

        // Inside the grace window the session still works — this is what
        // keeps an F5 reload alive while the new page restores.
        let body = r#"{"operation":"list_object_types","params":{}}"#;
        let still = raw_http(
            port,
            &format!(
                "POST /v1/apps/launch/{launch_id}/ark HTTP/1.1\r\n\
                 Host: 127.0.0.1:{port}\r\n\
                 X-Kosmos-Launch-Token: {launch_token}\r\n\
                 Content-Length: {}\r\n\
                 Connection: close\r\n\r\n{body}",
                body.len()
            ),
        )
        .await;
        assert_eq!(response_json(&still)["ok"], true, "{still}");

        // The reloaded page's renew cancels the release: a later release +
        // expiry would otherwise look identical to a real close.
        let renewed = raw_http(
            port,
            &format!(
                "POST /v1/apps/launch/{launch_id}/renew HTTP/1.1\r\n\
                 Host: 127.0.0.1:{port}\r\n\
                 X-Kosmos-Launch-Token: {launch_token}\r\n\
                 Content-Length: 0\r\n\
                 Connection: close\r\n\r\n"
            ),
        )
        .await;
        assert!(renewed.starts_with("HTTP/1.1 200"), "{renewed}");
        assert!(
            fixture
                .leases
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .live_grant(&launch_id, launch_token)
                .is_some(),
            "renew must cancel the release mark"
        );

        // A real close: release and let the grace lapse — the lease is gone.
        let release = raw_http(
            port,
            &format!(
                "POST /v1/apps/launch/{launch_id}/release HTTP/1.1\r\n\
                 Host: 127.0.0.1:{port}\r\n\
                 Content-Length: {}\r\n\
                 Connection: close\r\n\r\n{{\"token\":\"{launch_token}\"}}",
                format!(r#"{{"token":"{launch_token}"}}"#).len()
            ),
        )
        .await;
        assert!(release.starts_with("HTTP/1.1 200"), "{release}");
        fixture
            .leases
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .expire_release(&launch_id);
        let dead = raw_http(
            port,
            &format!(
                "POST /v1/apps/launch/{launch_id}/ark HTTP/1.1\r\n\
                 Host: 127.0.0.1:{port}\r\n\
                 X-Kosmos-Launch-Token: {launch_token}\r\n\
                 Content-Length: {}\r\n\
                 Connection: close\r\n\r\n{body}",
                body.len()
            ),
        )
        .await;
        assert!(dead.starts_with("HTTP/1.1 403"), "{dead}");

        // A wrong-token release is denied like a wrong revoke.
        let wrong = raw_http(
            port,
            &format!(
                "POST /v1/apps/launch/{launch_id}/release HTTP/1.1\r\n\
                 Host: 127.0.0.1:{port}\r\n\
                 Content-Length: 20\r\n\
                 Connection: close\r\n\r\n{{\"token\":\"deadbeef\"}}"
            ),
        )
        .await;
        assert!(wrong.starts_with("HTTP/1.1 403"), "{wrong}");

        fixture.shutdown().await;
    }
