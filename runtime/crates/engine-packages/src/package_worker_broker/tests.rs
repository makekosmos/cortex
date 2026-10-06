// fs_atomic items are only reached by the Windows-gated tests below; on
// other hosts the glob import is unused and trips -D warnings.
#[cfg(windows)]
use super::fs_atomic::*;
use super::fs_ops::*;
use super::fs_safety::*;
use super::net::*;
use super::secrets::*;
use super::snapshots::*;
use super::*;
use crate::package_manifest::IntegrationRequestMethod;
use std::{fs, path::Path};
#[test]
fn rejects_private_and_traversal() {
    assert!(is_blocked_ip("127.0.0.1".parse().unwrap()));
    assert!(is_blocked_ip("::ffff:127.0.0.1".parse().unwrap()));
    assert!(is_blocked_ip("::1".parse().unwrap()));
    assert!(is_blocked_ip("fc00::1".parse().unwrap()));
    assert!(is_blocked_ip("fe80::1".parse().unwrap()));
    assert!(reject_path(Path::new("relative")).is_err());
    assert!(reject_path(Path::new(r"C:\\root\\..\\escape")).is_err());
    assert!(reject_path(Path::new(r"\\server\\share\\file")).is_err());
    assert!(reject_path(Path::new(r"\\.\PIPE\name")).is_err());
}

#[test]
fn validates_https_origin_boundary() {
    let cfg = BrokerConfig::new(["https://example.com"], Vec::new()).unwrap();
    assert!(validate_url(&cfg, "https://example.com/a").is_ok());
    assert!(validate_url(&cfg, "http://example.com/a").is_err());
    assert!(validate_url(&cfg, "https://user@example.com/a").is_err());
    assert!(validate_url(&cfg, "https://example.net/a").is_err());
    assert!(validate_url(&cfg, "https://localhost/a").is_err());
}
#[test]
fn json_session_injection_preserves_worker_body_and_hides_unmapped_fields() {
    let injection: SecretInjection = serde_json::from_value(serde_json::json!({
        "kind": "json", "origins": ["https://example.com"],
        "body_fields": {"token": "accessToken"},
        "header_fields": {"x-huid": "uid"}, "headers": {"x-version": "test"}
    }))
    .unwrap();
    let secret = SecretRequest {
        injection: &injection,
        secret: r#"{"accessToken":"access","uid":"account","refreshToken":"private"}"#,
        allowed_cookie_names: &[],
    };
    let body = serde_json::json!({"version": 42});
    let injected = inject_json_body(Some(&body), Some(&secret))
        .unwrap()
        .unwrap();
    assert_eq!(
        injected,
        serde_json::json!({"version": 42, "token": "access"})
    );
    assert_eq!(body, serde_json::json!({"version": 42}));
    let bytes = request_body(injection.request_method(), Some(&injected))
        .unwrap()
        .unwrap();
    let request = apply_secret(
        engine_base::http::client()
            .post("https://example.com")
            .body(bytes),
        &secret,
    )
    .unwrap()
    .build()
    .unwrap();
    assert_eq!(request.headers()["x-huid"], "account");
    assert_eq!(request.headers()["x-version"], "test");
    assert!(
        !String::from_utf8_lossy(request.body().unwrap().as_bytes().unwrap()).contains("private")
    );
    assert!(inject_json_body(
        Some(&serde_json::json!({"token":"override"})),
        Some(&secret)
    )
    .is_err());
    assert!(inject_json_body(Some(&serde_json::json!([])), Some(&secret)).is_err());
    let missing = SecretRequest {
        secret: r#"{"uid":"account"}"#,
        ..secret
    };
    assert!(inject_json_body(Some(&body), Some(&missing)).is_err());
    let invalid = SecretRequest {
        secret: r#"{"uid":"bad\r\nheader","accessToken":"a"}"#,
        ..missing
    };
    assert!(apply_secret(
        engine_base::http::client().post("https://example.com"),
        &invalid
    )
    .is_err());
}

#[test]
fn network_response_buffers_are_bounded_and_owner_scoped() {
    let responses = SnapshotRegistry::network_responses();
    let data = vec![42; 25 * 1024 * 1024];
    let handle = responses
        .reserve("generation-1", "pkg", "url", data.clone())
        .unwrap();
    assert!(responses
        .reserve("generation-1", "pkg", "url", vec![])
        .is_err());
    assert!(responses.chunk(&handle, "generation-2", 0, 1).is_err());
    assert!(responses.close(&handle, "generation-2").is_err());
    assert!(responses
        .chunk(&handle, "generation-1", 0, MAX_SNAPSHOT_CHUNK + 1)
        .is_err());
    let mut restored = Vec::new();
    while restored.len() < data.len() {
        restored.extend(
            responses
                .chunk(&handle, "generation-1", restored.len(), MAX_SNAPSHOT_CHUNK)
                .unwrap(),
        );
    }
    assert_eq!(restored, data);
    for owner in ["b", "c", "d"] {
        responses.reserve(owner, "pkg", "url", vec![]).unwrap();
    }
    assert!(responses.reserve("e", "pkg", "url", vec![]).is_err());
    responses.close_owner("generation-1");
    assert!(responses.chunk(&handle, "generation-1", 0, 1).is_err());
    assert!(responses.reserve("e", "pkg", "url", vec![]).is_ok());
    assert!(SnapshotRegistry::new()
        .reserve("a", "pkg", "url", data)
        .is_err());
}

#[cfg(feature = "package-worker-fixture")]
#[tokio::test]
async fn network_response_http_limits_reject_oversized_content() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    for limit in [MAX_BYTES, MAX_NETWORK_RESPONSE] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0; 4096];
            assert!(socket.read(&mut request).await.unwrap() > 0);
            socket
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        limit + 1
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
        });
        let config = BrokerConfig::new([origin.clone()], vec![])
            .unwrap()
            .enable_local_test_origin();
        let error = fetch_with_secret_json_limit(&config, &origin, None, None, limit)
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            BrokerError::Invalid(message) if message == "response exceeds limit",
        ));
        server.await.unwrap();
    }
}

#[test]
fn secret_injection_is_manifest_bound_and_filters_cookies() {
    let client = engine_base::http::client();
    let injection = SecretInjection::Header {
        origins: vec!["https://example.com/".into()],
        name: "Authorization".into(),
        prefix: "Bearer ".into(),
    };
    let request = apply_secret(
        client.get("https://example.com/data"),
        &SecretRequest {
            injection: &injection,
            secret: "token",
            allowed_cookie_names: &[],
        },
    )
    .unwrap()
    .build()
    .unwrap();
    assert_eq!(request.headers()["authorization"], "Bearer token");
    assert!(reqwest::Url::parse(&injection.origins()[0])
        .is_ok_and(|allowed| allowed.origin() == request.url().origin()));

    let cookies = SecretInjection::Cookies {
        origins: vec!["https://example.com/".into()],
        method: IntegrationRequestMethod::PostJson,
        headers: [("origin".into(), "https://example.com".into())].into(),
        header_from_cookie: Some(crate::package_manifest::CookieHeaderInjection {
            cookie: "csrf".into(),
            header: "x-csrf-token".into(),
        }),
    };
    assert!(apply_secret(
        client.get("https://example.com/data"),
        &SecretRequest {
            injection: &cookies,
            secret: r#"{"other":"leak"}"#,
            allowed_cookie_names: &["session".into()],
        },
    )
    .is_err());
    let request = apply_secret(
        client.post("https://example.com/data"),
        &SecretRequest {
            injection: &cookies,
            secret: r#"{"session":"opaque","csrf":"mirror"}"#,
            allowed_cookie_names: &["session".into(), "csrf".into()],
        },
    )
    .unwrap()
    .build()
    .unwrap();
    assert_eq!(request.headers()["origin"], "https://example.com");
    assert_eq!(request.headers()["x-csrf-token"], "mirror");
    assert!(request.headers()["cookie"]
        .to_str()
        .unwrap()
        .contains("session=opaque"));
    assert!(request_body(IntegrationRequestMethod::Get, Some(&serde_json::json!({}))).is_err());
    assert!(request_body(
        IntegrationRequestMethod::PostJson,
        Some(&serde_json::json!({ "body": "x".repeat(MAX_JSON_BODY) }))
    )
    .is_err());
}
#[test]
fn snapshot_lifecycle_returns_opaque_bounded_chunks_and_closes_by_owner() {
    let snapshots = SnapshotRegistry::new();
    let handle = snapshots
        .reserve(
            "desktop-generation-1",
            "eden",
            "bundled",
            b"abcdef".to_vec(),
        )
        .unwrap();
    assert!(!handle.contains('/'));
    assert_eq!(
        snapshots
            .chunk(&handle, "desktop-generation-1", 0, 3)
            .unwrap(),
        b"abc"
    );
    assert_eq!(
        snapshots
            .chunk(&handle, "desktop-generation-1", 3, 3)
            .unwrap(),
        b"def"
    );
    assert!(snapshots.chunk(&handle, "other-generation", 0, 3).is_err());
    snapshots.close_owner("desktop-generation-1");
    assert!(snapshots
        .chunk(&handle, "desktop-generation-1", 0, 3)
        .is_err());
}

#[test]
fn snapshots_real_bytes_and_rejects_nested_symlink() {
    let td = tempfile::tempdir().unwrap();
    let package = td.path().join("pkg");
    fs::create_dir(&package).unwrap();
    fs::write(package.join("manifest.json"), br#"{"id":"pkg"}"#).unwrap();
    fs::create_dir(package.join("dist")).unwrap();
    fs::write(package.join("dist/index.html"), b"immutable").unwrap();
    let cfg = BrokerConfig::new(std::iter::empty::<&str>(), vec![td.path().to_path_buf()]).unwrap();
    let files = read_snapshot_tree(&cfg, &package).unwrap();
    assert_eq!(
        files
            .iter()
            .find(|file| file.path == "dist/index.html")
            .unwrap()
            .bytes,
        b"immutable"
    );
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(td.path(), package.join("nested-link")).unwrap();
        assert!(read_snapshot_tree(&cfg, &package).is_err());
    }
}

#[test]
fn reads_and_writes_under_root() {
    let td = tempfile::tempdir().unwrap();
    let cfg = BrokerConfig::new(std::iter::empty::<&str>(), vec![td.path().to_path_buf()]).unwrap();
    let nested = td.path().join("nested/deep");
    create_directory(&cfg, &nested).unwrap();
    let p = nested.join("x");
    write_file(&cfg, &p, b"ok").unwrap();
    assert_eq!(read_file(&cfg, &p).unwrap(), b"ok");
    write_file(&cfg, &p, b"second").unwrap();
    assert_eq!(read_file(&cfg, &p).unwrap(), b"second");
    assert!(create_directory(&cfg, &tempfile::tempdir().unwrap().path().join("outside")).is_err());
}

#[test]
fn rejects_oversized_and_out_of_root_files() {
    let td = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let cfg = BrokerConfig::new(std::iter::empty::<&str>(), vec![td.path().to_path_buf()]).unwrap();
    assert!(write_file(&cfg, &td.path().join("large"), &vec![0; MAX_BYTES + 1]).is_err());
    let outside_file = outside.path().join("x");
    fs::write(&outside_file, b"x").unwrap();
    assert!(read_file(&cfg, &outside_file).is_err());
}

#[cfg(unix)]
#[test]
fn rejects_symlink_escape() {
    use std::os::unix::fs::symlink;

    let td = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let cfg = BrokerConfig::new(std::iter::empty::<&str>(), vec![td.path().to_path_buf()]).unwrap();
    let outside_file = outside.path().join("x");
    fs::write(&outside_file, b"x").unwrap();
    let link = td.path().join("link");
    symlink(&outside_file, &link).unwrap();
    assert!(read_file(&cfg, &link).is_err());
    assert!(write_file(&cfg, &link, b"nope").is_err());
}

#[cfg(unix)]
#[test]
fn detects_path_replacement_after_open() {
    use std::os::unix::fs::MetadataExt;

    let td = tempfile::tempdir().unwrap();
    let cfg = BrokerConfig::new(std::iter::empty::<&str>(), vec![td.path().to_path_buf()]).unwrap();
    let path = td.path().join("x");
    fs::write(&path, b"old").unwrap();
    let file = open_existing_target(&path).unwrap();
    let metadata = file.metadata().unwrap();
    fs::rename(&path, td.path().join("old")).unwrap();
    fs::write(&path, b"new").unwrap();
    assert!(validate_open_file(&cfg, &path, &file, &metadata).is_err());
    assert_ne!(metadata.ino(), fs::metadata(&path).unwrap().ino());
}

#[cfg(windows)]
#[test]
fn creates_temp_file_relative_to_validated_parent_handle() {
    let root = tempfile::tempdir().unwrap();
    let original = root.path().join("original");
    let moved = root.path().join("moved");
    fs::create_dir(&original).unwrap();
    let cfg =
        BrokerConfig::new(std::iter::empty::<&str>(), vec![root.path().to_path_buf()]).unwrap();
    let (parent_guard, canonical) = open_parent_dir(&cfg, &original).unwrap();

    // Replace the path after validation.  A path-based CreateFile would
    // follow this new directory; the retained handle must continue to the
    // original directory now reachable through `moved`.
    fs::rename(&original, &moved).unwrap();
    fs::create_dir(&original).unwrap();

    let (temp_path, temp) = create_temp_file(&canonical, parent_guard.as_ref()).unwrap();
    let name = temp_path.file_name().unwrap().to_owned();
    drop(temp);
    assert!(moved.join(&name).is_file());
    assert!(!original.join(&name).exists());
}

// KOS-270: opening the parent dir used to request FILE_DELETE_CHILD +
// DELETE, which a plain "Modify" grant does not include — writes into a
// real user folder denied by `open_parent_dir`. The broker must work with
// Modify: this test locks the fixture dir down to Modify only.
#[cfg(windows)]
#[test]
fn broker_file_ops_succeed_under_modify_only_acl() {
    let td = tempfile::tempdir().unwrap();
    let guarded = td.path().join("guarded");
    fs::create_dir(&guarded).unwrap();
    let user = std::env::var("USERNAME").expect("USERNAME");
    let status = std::process::Command::new("icacls")
        .arg(&guarded)
        .args(["/inheritance:r", "/grant:r"])
        .arg(format!("{user}:(OI)(CI)M"))
        .status()
        .expect("icacls");
    assert!(status.success(), "icacls failed");
    let cfg = BrokerConfig::new(std::iter::empty::<&str>(), vec![guarded.clone()]).unwrap();
    let nested = guarded.join("nested/deep");
    create_directory(&cfg, &nested).unwrap();
    let file = nested.join("note.txt");
    write_file(&cfg, &file, b"one").unwrap();
    assert_eq!(read_file(&cfg, &file).unwrap(), b"one");
    write_file(&cfg, &file, b"two").unwrap();
    assert_eq!(read_file(&cfg, &file).unwrap(), b"two");
    delete_file(&cfg, &file).unwrap();
    assert!(read_file(&cfg, &file).is_err());
}
