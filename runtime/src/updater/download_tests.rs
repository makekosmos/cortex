use sha2::Digest;

use super::*;
use httpmock::MockServer;

fn hash(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(Sha512::digest(bytes))
}

#[tokio::test]
async fn downloads_fresh_file() {
    let body = b"mundus installer bytes".repeat(100);
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(httpmock::Method::GET).path("/installer.exe");
            then.status(200).body(body.clone());
        })
        .await;
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("installer.exe.part");
    let mut last = (0, 0);
    download_resumable(
        &reqwest::Client::new(),
        &server.url("/installer.exe"),
        &dest,
        body.len() as u64,
        |done, total| last = (done, total),
    )
    .await
    .unwrap();
    assert_eq!(last, (body.len() as u64, body.len() as u64));
    assert_eq!(tokio::fs::read(dest).await.unwrap(), body);
}

#[tokio::test]
async fn resumes_with_range() {
    let body = b"mundus installer bytes".repeat(100);
    let split = body.len() / 2;
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(httpmock::Method::GET)
                .path("/installer.exe")
                .header("Range", format!("bytes={split}-"));
            then.status(206).body(&body[split..]);
        })
        .await;
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("installer.exe.part");
    tokio::fs::write(&dest, &body[..split]).await.unwrap();
    download_resumable(
        &reqwest::Client::new(),
        &server.url("/installer.exe"),
        &dest,
        body.len() as u64,
        |_, _| {},
    )
    .await
    .unwrap();
    assert_eq!(tokio::fs::read(dest).await.unwrap(), body);
}

#[tokio::test]
async fn ignored_range_restarts_partial_file() {
    let body = b"mundus installer bytes".repeat(100);
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.method(httpmock::Method::GET).path("/installer.exe");
            then.status(200).body(body.clone());
        })
        .await;
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("installer.exe.part");
    tokio::fs::write(&dest, b"partial").await.unwrap();
    download_resumable(
        &reqwest::Client::new(),
        &server.url("/installer.exe"),
        &dest,
        body.len() as u64,
        |_, _| {},
    )
    .await
    .unwrap();
    assert_eq!(tokio::fs::read(dest).await.unwrap(), body);
}

#[tokio::test]
async fn network_failure_preserves_error_kind() {
    let dir = tempfile::tempdir().unwrap();
    let error = download_resumable(
        &reqwest::Client::new(),
        "http://127.0.0.1:1/installer.exe",
        &dir.path().join("installer.exe.part"),
        10,
        |_, _| {},
    )
    .await
    .unwrap_err();
    assert!(matches!(error, UpdaterError::Network(_)));
}

#[tokio::test]
async fn verification_checks_size_and_hash() {
    let body = b"mundus installer payload";
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("installer.exe.part");
    tokio::fs::write(&path, body).await.unwrap();
    verify_file(&path, body.len() as u64, &hash(body))
        .await
        .unwrap();
    assert!(matches!(
        verify_file(&path, 999, &hash(body)).await,
        Err(UpdaterError::SizeMismatch { .. })
    ));
    assert!(matches!(
        verify_file(&path, body.len() as u64, &hash(b"different")).await,
        Err(UpdaterError::HashMismatch)
    ));
}

#[tokio::test]
async fn verified_part_is_atomically_committed() {
    let body = b"verified installer";
    let dir = tempfile::tempdir().unwrap();
    let part = dir.path().join("installer.exe.part");
    let final_path = dir.path().join("installer.exe");
    tokio::fs::write(&part, body).await.unwrap();
    verify_and_commit(&part, &final_path, body.len() as u64, &hash(body))
        .await
        .unwrap();
    assert!(!part.exists());
    assert_eq!(tokio::fs::read(final_path).await.unwrap(), body);
}
