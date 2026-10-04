mod releases_tests {
    use super::releases::*;
    use super::{app_descriptor, NativeAppDescriptor};
    use httpmock::Method::GET;

    const AGENDA: &NativeAppDescriptor = &super::NATIVE_APPS[0];
    const TARGET: &str = "x86_64-pc-windows-msvc";

    fn sums(version: &str, sha: &str) -> String {
        let stem = AGENDA.asset_stem;
        format!(
            "{sha}  {stem}-{version}-x86_64-pc-windows-msvc.zip\n\
             {sha}  {stem}-{version}-aarch64-apple-darwin.tar.gz\n\
             {sha} *{stem}-{version}-x86_64-unknown-linux-gnu.tar.gz\n"
        )
    }

    fn sums_path(tag: &str) -> String {
        format!(
            "/{}/releases/download/{tag}/SHA256SUMS.txt",
            AGENDA.repository
        )
    }

    async fn stub_latest(server: &httpmock::MockServer, tag: &str) {
        server
            .mock_async(|when, then| {
                when.method(GET).path(format!(
                    "/{}/releases/latest/download/SHA256SUMS.txt",
                    AGENDA.repository
                ));
                then.status(302).header(
                    "Location",
                    format!("{}{}", server.base_url(), sums_path(tag)),
                );
            })
            .await;
    }

    async fn stub_sums(server: &httpmock::MockServer, tag: &str, body: String, etag: Option<&str>) {
        server
            .mock_async(|when, then| {
                when.method(GET).path(sums_path(tag));
                let then = then.status(200).body(body);
                match etag {
                    Some(etag) => then.header("ETag", etag),
                    None => then,
                };
            })
            .await;
    }

    #[test]
    fn parse_sums_reads_sha256sum_format() {
        let body = "abcd  one.zip\n\n1234 *two.tar.gz\njunk line\n";
        // "abcd"/"1234" aren't 64-hex — nothing parses.
        assert!(parse_sums(body).is_empty());
        let body = format!("{}  a.zip\n", "a".repeat(64));
        assert_eq!(parse_sums(&body)[0].name, "a.zip");
        let body = format!("{} *b.tar.gz\n", "b".repeat(64));
        assert_eq!(parse_sums(&body)[0].name, "b.tar.gz");
    }

    #[test]
    fn select_asset_picks_the_host_target_line() {
        let parsed = parse_sums(&sums("0.3.0", &"a".repeat(64)));
        let found = select_asset(&parsed, AGENDA, TARGET).expect("windows line");
        assert_eq!(found.version, "0.3.0");
        assert_eq!(found.asset, "agenda-gpui-0.3.0-x86_64-pc-windows-msvc.zip");
        assert_eq!(found.sha256, "a".repeat(64));
        // Other targets resolve to their own extension.
        let found = select_asset(&parsed, AGENDA, "aarch64-apple-darwin").expect("darwin line");
        assert_eq!(found.version, "0.3.0");
        assert!(found.asset.ends_with(".tar.gz"));
        // Absent target → None.
        assert!(select_asset(&parsed, AGENDA, "x86_64-pc-windows-gnu").is_none());
        // Wrong stem → None.
        let dictation = app_descriptor("com.kosmos.dictation").unwrap();
        assert!(select_asset(&parsed, dictation, TARGET).is_none());
    }

    #[test]
    fn tag_round_trip_per_app() {
        for desc in super::NATIVE_APPS {
            let tag = desc.release_tag("0.3.0");
            assert_eq!(desc.version_from_tag(&tag).as_deref(), Some("0.3.0"));
            // A foreign prefix must not parse.
            let other = if desc.tag_prefix == "v" {
                "gpui-v0.3.0"
            } else {
                "v0.3.0"
            };
            if desc.tag_prefix == "gpui-v" {
                assert!(desc.version_from_tag(other).is_none());
            } else {
                assert!(desc.version_from_tag("vX.Y").is_none());
            }
        }
    }

    #[test]
    fn info_from_sums_rejects_tag_version_mismatch() {
        let body = sums("0.3.0", &"a".repeat(64));
        // Tag says v9.9.9, asset says 0.3.0 → cross-check fails closed.
        assert!(info_from_sums(AGENDA, "v9.9.9", &body, TARGET).is_err());
        let info = info_from_sums(AGENDA, "v0.3.0", &body, TARGET).unwrap();
        assert_eq!(info.tag, "v0.3.0");
        assert_eq!(info.version, "0.3.0");
        assert_eq!(info.asset, "agenda-gpui-0.3.0-x86_64-pc-windows-msvc.zip");
    }

    #[tokio::test]
    async fn fetch_latest_follows_tag_redirect() {
        let server = httpmock::MockServer::start_async().await;
        let sha = "a".repeat(64);
        stub_latest(&server, "v0.1.1").await;
        stub_sums(
            &server,
            "v0.1.1",
            format!("{sha}  agenda-gpui-0.1.1-x86_64-pc-windows-msvc.zip\n"),
            Some("\"sums-v0.1.1\""),
        )
        .await;
        let probe = ReleaseProbe::with_base(server.base_url()).unwrap();
        let result = fetch_latest(&probe, AGENDA, TARGET, None).await;
        let Ok(ReleaseCheck::Fresh { info, etag }) = result else {
            panic!("expected fresh release, got {result:?}")
        };
        assert_eq!(info.tag, "v0.1.1");
        assert_eq!(info.version, "0.1.1");
        assert_eq!(info.asset, "agenda-gpui-0.1.1-x86_64-pc-windows-msvc.zip");
        assert_eq!(info.sha256, sha);
        assert_eq!(etag.as_deref(), Some("\"sums-v0.1.1\""));
    }

    #[tokio::test]
    async fn fetch_latest_revalidates_with_etag() {
        let server = httpmock::MockServer::start_async().await;
        stub_latest(&server, "v0.1.1").await;
        server
            .mock_async(|when, then| {
                when.method(GET)
                    .path(sums_path("v0.1.1"))
                    .header("if-none-match", "\"sums-v0.1.1\"");
                then.status(304);
            })
            .await;
        let probe = ReleaseProbe::with_base(server.base_url()).unwrap();
        let cached = CachedReleaseMeta {
            tag: "v0.1.1".into(),
            etag: "\"sums-v0.1.1\"".into(),
        };
        assert!(matches!(
            fetch_latest(&probe, AGENDA, TARGET, Some(&cached)).await,
            Ok(ReleaseCheck::NotModified)
        ));
    }

    #[tokio::test]
    async fn not_modified_for_a_different_tag_refetches() {
        // The release moved to v0.2.0 but the server answers 304 to the old
        // etag — the cache must not claim "latest" for the wrong tag.
        let server = httpmock::MockServer::start_async().await;
        stub_latest(&server, "v0.2.0").await;
        server
            .mock_async(|when, then| {
                when.method(GET)
                    .path(sums_path("v0.2.0"))
                    .header_exists("if-none-match");
                then.status(304);
            })
            .await;
        let sha = "b".repeat(64);
        stub_sums(
            &server,
            "v0.2.0",
            format!("{sha}  agenda-gpui-0.2.0-x86_64-pc-windows-msvc.zip\n"),
            None,
        )
        .await;
        let probe = ReleaseProbe::with_base(server.base_url()).unwrap();
        let stale = CachedReleaseMeta {
            tag: "v0.1.1".into(),
            etag: "\"sums-v0.1.1\"".into(),
        };
        let Ok(ReleaseCheck::Fresh { info, .. }) =
            fetch_latest(&probe, AGENDA, TARGET, Some(&stale)).await
        else {
            panic!("moved tag must refetch the body")
        };
        assert_eq!(info.tag, "v0.2.0");
        assert_eq!(info.version, "0.2.0");
    }

    #[tokio::test]
    async fn direct_body_for_latest_is_rejected() {
        // No redirect hop = no authoritative tag — fail closed instead of
        // trusting an untagged body.
        let server = httpmock::MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(GET).path(format!(
                    "/{}/releases/latest/download/SHA256SUMS.txt",
                    AGENDA.repository
                ));
                then.status(200).body(sums("0.1.1", &"a".repeat(64)));
            })
            .await;
        let probe = ReleaseProbe::with_base(server.base_url()).unwrap();
        assert!(matches!(
            fetch_latest(&probe, AGENDA, TARGET, None).await,
            Err(ReleaseError::Unavailable)
        ));
    }

    #[tokio::test]
    async fn fetch_latest_fails_closed_offline_and_on_mismatch() {
        // Unreachable base → Unavailable, never a panic.
        let probe = ReleaseProbe::with_base("http://127.0.0.1:9".into()).unwrap();
        assert!(matches!(
            fetch_latest(&probe, AGENDA, TARGET, None).await,
            Err(ReleaseError::Unavailable)
        ));

        // Redirect tag disagrees with the sums' asset version → Invalid.
        let server = httpmock::MockServer::start_async().await;
        stub_latest(&server, "v9.9.9").await;
        stub_sums(
            &server,
            "v9.9.9",
            format!(
                "{}  agenda-gpui-0.1.1-x86_64-pc-windows-msvc.zip\n",
                "a".repeat(64)
            ),
            None,
        )
        .await;
        let probe = ReleaseProbe::with_base(server.base_url()).unwrap();
        assert!(matches!(
            fetch_latest(&probe, AGENDA, TARGET, None).await,
            Err(ReleaseError::Invalid("tag/version mismatch"))
        ));
    }

    #[tokio::test]
    async fn oversized_sums_body_is_rejected() {
        let server = httpmock::MockServer::start_async().await;
        stub_latest(&server, "v0.1.1").await;
        stub_sums(&server, "v0.1.1", "x".repeat(128 * 1024), None).await;
        let probe = ReleaseProbe::with_base(server.base_url()).unwrap();
        assert!(matches!(
            fetch_latest(&probe, AGENDA, TARGET, None).await,
            Err(ReleaseError::Invalid("sums too large"))
        ));
    }

    #[test]
    fn tag_from_location_only_accepts_same_host_release_path() {
        let base = reqwest::Url::parse("https://github.com").unwrap();
        assert_eq!(
            tag_from_location(
                &base,
                "makekosmos/agenda-gpui",
                "https://github.com/makekosmos/agenda-gpui/releases/download/v0.3.0/SHA256SUMS.txt",
            )
            .as_deref(),
            Some("v0.3.0"),
        );
        // Root-relative location resolves against the base.
        assert_eq!(
            tag_from_location(
                &base,
                "makekosmos/agenda-gpui",
                "/makekosmos/agenda-gpui/releases/download/v0.3.0/SHA256SUMS.txt",
            )
            .as_deref(),
            Some("v0.3.0"),
        );
        // Other host, other repo, wrong asset → None.
        assert!(tag_from_location(
            &base,
            "makekosmos/agenda-gpui",
            "https://evil.example/makekosmos/agenda-gpui/releases/download/v0.3.0/SHA256SUMS.txt",
        )
        .is_none());
        assert!(tag_from_location(
            &base,
            "makekosmos/agenda-gpui",
            "https://github.com/makekosmos/other/releases/download/v0.3.0/SHA256SUMS.txt",
        )
        .is_none());
        assert!(tag_from_location(
            &base,
            "makekosmos/agenda-gpui",
            "https://github.com/makekosmos/agenda-gpui/releases/download/v0.3.0/EVIL.txt",
        )
        .is_none());
        // Same host over plain http is a downgrade, not a valid tag hop.
        assert!(tag_from_location(
            &base,
            "makekosmos/agenda-gpui",
            "http://github.com/makekosmos/agenda-gpui/releases/download/v0.3.0/SHA256SUMS.txt",
        )
        .is_none());
    }

    #[test]
    fn base_must_be_a_bare_origin() {
        for good in ["https://github.com", "https://github.com/"] {
            assert!(
                ReleaseProbe::with_base(good.to_owned()).is_ok(),
                "{good} must be accepted"
            );
        }
        // Path, credentials and query all disqualify the base — each would
        // corrupt the URL joins the probe builds on it.
        for bad in [
            "https://github.com/extra",
            "https://user:pw@github.com",
            "https://github.com?x=1",
            "not a url",
        ] {
            assert!(
                ReleaseProbe::with_base(bad.to_owned()).is_err(),
                "{bad} must be rejected"
            );
        }
    }

    #[tokio::test]
    async fn fetch_tagged_reads_sums_for_the_named_tag() {
        // A pinned tag skips the latest redirect entirely and still gets the
        // tag/asset cross-check.
        let server = httpmock::MockServer::start_async().await;
        stub_sums(
            &server,
            "v0.1.1",
            sums("0.1.1", "aa".repeat(32).as_str()),
            None,
        )
        .await;
        let probe = ReleaseProbe::with_base(server.base_url()).unwrap();
        let info = fetch_tagged(&probe, AGENDA, "v0.1.1", TARGET)
            .await
            .expect("tagged fetch");
        assert_eq!(info.tag, "v0.1.1");
        assert_eq!(info.version, "0.1.1");
        assert!(fetch_tagged(&probe, AGENDA, "v9.9.9", TARGET)
            .await
            .is_err());
    }
}
