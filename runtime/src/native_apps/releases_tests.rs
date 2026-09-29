mod releases_tests {
    use super::releases::*;
    use super::{app_descriptor, NativeAppDescriptor};

    const AGENDA: &NativeAppDescriptor = &super::NATIVE_APPS[0];

    fn sums(version: &str, sha: &str) -> String {
        let stem = AGENDA.asset_stem;
        format!(
            "{sha}  {stem}-{version}-x86_64-pc-windows-msvc.zip\n\
             {sha}  {stem}-{version}-aarch64-apple-darwin.tar.gz\n\
             {sha} *{stem}-{version}-x86_64-unknown-linux-gnu.tar.gz\n"
        )
    }

    #[test]
    fn parse_sums_reads_sha256sum_format() {
        let body = "abcd  one.zip\n\n1234 *two.tar.gz\njunk line\n";
        let parsed = parse_sums(body);
        // "abcd"/"1234" aren't 64-hex — nothing parses.
        assert!(parsed.is_empty());
        let body = format!("{}  a.zip\n", "a".repeat(64));
        assert_eq!(parse_sums(&body)[0].1, "a.zip");
        let body = format!("{} *b.tar.gz\n", "b".repeat(64));
        assert_eq!(parse_sums(&body)[0].1, "b.tar.gz");
    }

    #[test]
    fn select_asset_picks_the_host_target_line() {
        let parsed = parse_sums(&sums("0.3.0", &"a".repeat(64)));
        let (version, name, sha) =
            select_asset(&parsed, AGENDA, "x86_64-pc-windows-msvc").expect("windows line");
        assert_eq!(version, "0.3.0");
        assert_eq!(name, "agenda-gpui-0.3.0-x86_64-pc-windows-msvc.zip");
        assert_eq!(sha, "a".repeat(64));
        // Other targets resolve to their own extension.
        let (version, name, _) =
            select_asset(&parsed, AGENDA, "aarch64-apple-darwin").expect("darwin line");
        assert_eq!(version, "0.3.0");
        assert!(name.ends_with(".tar.gz"));
        // Absent target → None.
        assert!(select_asset(&parsed, AGENDA, "x86_64-pc-windows-gnu").is_none());
        // Wrong stem → None.
        let dictation = app_descriptor("com.kosmos.dictation").unwrap();
        assert!(select_asset(&parsed, dictation, "x86_64-pc-windows-msvc").is_none());
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
                // `gpui-v` apps must not accept a bare `v` tag.
                assert!(desc.version_from_tag(other).is_none());
            } else {
                // `v` prefix + non-semver tail → None.
                assert!(desc.version_from_tag("vX.Y").is_none());
            }
        }
    }

    #[test]
    fn info_from_sums_rejects_tag_version_mismatch() {
        let body = sums("0.3.0", &"a".repeat(64));
        // Tag says v9.9.9, asset says 0.3.0 → cross-check fails closed.
        assert!(info_from_sums(AGENDA, Some("v9.9.9"), &body, "x86_64-pc-windows-msvc").is_err());
        let info = info_from_sums(AGENDA, Some("v0.3.0"), &body, "x86_64-pc-windows-msvc").unwrap();
        assert_eq!(info.tag, "v0.3.0");
        assert_eq!(info.version, "0.3.0");
        assert_eq!(info.asset, "agenda-gpui-0.3.0-x86_64-pc-windows-msvc.zip");
        // No tag (direct-body stub) → tag rebuilt from prefix + version.
        let info = info_from_sums(AGENDA, None, &body, "x86_64-pc-windows-msvc").unwrap();
        assert_eq!(info.tag, "v0.3.0");
    }

    #[tokio::test]
    async fn fetch_latest_follows_tag_redirect() {
        let server = httpmock::MockServer::start_async().await;
        let base = server.base_url();
        let sha = "a".repeat(64);
        let sums = format!("{sha}  agenda-gpui-0.1.1-x86_64-pc-windows-msvc.zip\n");
        server
            .mock_async(|when, then| {
                when.method(httpmock::Method::GET)
                    .path("/makekosmos/agenda-gpui/releases/latest/download/SHA256SUMS.txt");
                then.status(302).header(
                    "Location",
                    format!(
                        "{base}/makekosmos/agenda-gpui/releases/download/v0.1.1/SHA256SUMS.txt"
                    ),
                );
            })
            .await;
        server
            .mock_async(|when, then| {
                when.method(httpmock::Method::GET)
                    .path("/makekosmos/agenda-gpui/releases/download/v0.1.1/SHA256SUMS.txt");
                then.status(200)
                    .header("ETag", "\"sums-v0.1.1\"")
                    .body(sums);
            })
            .await;
        let probe = ReleaseProbe::with_base(base).unwrap();
        let result = fetch_latest(&probe, AGENDA, "x86_64-pc-windows-msvc", None).await;
        assert!(matches!(&result, Ok(ReleaseCheck::Fresh { .. })));
        let Ok(ReleaseCheck::Fresh { info, etag }) = result else {
            return;
        };
        assert_eq!(info.tag, "v0.1.1");
        assert_eq!(info.version, "0.1.1");
        assert_eq!(info.asset, "agenda-gpui-0.1.1-x86_64-pc-windows-msvc.zip");
        assert_eq!(info.sha256, "a".repeat(64));
        assert_eq!(etag.as_deref(), Some("\"sums-v0.1.1\""));
    }

    #[tokio::test]
    async fn fetch_latest_revalidates_with_etag() {
        let server = httpmock::MockServer::start_async().await;
        let base = server.base_url();
        server
            .mock_async(|when, then| {
                when.method(httpmock::Method::GET)
                    .path("/makekosmos/agenda-gpui/releases/latest/download/SHA256SUMS.txt");
                then.status(302).header(
                    "Location",
                    format!(
                        "{base}/makekosmos/agenda-gpui/releases/download/v0.1.1/SHA256SUMS.txt"
                    ),
                );
            })
            .await;
        server
            .mock_async(|when, then| {
                when.method(httpmock::Method::GET)
                    .path("/makekosmos/agenda-gpui/releases/download/v0.1.1/SHA256SUMS.txt")
                    .header("if-none-match", "\"sums-v0.1.1\"");
                then.status(304);
            })
            .await;
        let probe = ReleaseProbe::with_base(base).unwrap();
        let result = fetch_latest(
            &probe,
            AGENDA,
            "x86_64-pc-windows-msvc",
            Some("\"sums-v0.1.1\""),
        )
        .await;
        assert!(matches!(result, Ok(ReleaseCheck::NotModified)));
    }

    #[tokio::test]
    async fn fetch_latest_fails_closed_offline_and_on_mismatch() {
        // Unreachable base → Unavailable, never a panic.
        let probe = ReleaseProbe::with_base("http://127.0.0.1:9".into()).unwrap();
        assert!(matches!(
            fetch_latest(&probe, AGENDA, "x86_64-pc-windows-msvc", None).await,
            Err(ReleaseError::Unavailable)
        ));

        // Redirect tag disagrees with the sums' asset version → Invalid.
        let server = httpmock::MockServer::start_async().await;
        let base = server.base_url();
        server
            .mock_async(|when, then| {
                when.method(httpmock::Method::GET)
                    .path("/makekosmos/agenda-gpui/releases/latest/download/SHA256SUMS.txt");
                then.status(302).header(
                    "Location",
                    format!(
                        "{base}/makekosmos/agenda-gpui/releases/download/v9.9.9/SHA256SUMS.txt"
                    ),
                );
            })
            .await;
        server
            .mock_async(|when, then| {
                when.method(httpmock::Method::GET)
                    .path("/makekosmos/agenda-gpui/releases/download/v9.9.9/SHA256SUMS.txt");
                then.status(200).body(format!(
                    "{}  agenda-gpui-0.1.1-x86_64-pc-windows-msvc.zip\n",
                    "a".repeat(64)
                ));
            })
            .await;
        let probe = ReleaseProbe::with_base(base).unwrap();
        assert!(matches!(
            fetch_latest(&probe, AGENDA, "x86_64-pc-windows-msvc", None).await,
            Err(ReleaseError::Invalid("tag/version mismatch"))
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
        // Other host, other repo, traversal-ish tag, wrong asset → None.
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
    }
}
