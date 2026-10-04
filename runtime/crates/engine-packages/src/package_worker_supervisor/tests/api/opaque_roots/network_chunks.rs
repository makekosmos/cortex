#[cfg(feature = "package-worker-fixture")]
use super::*;

#[cfg(feature = "package-worker-fixture")]
#[tokio::test]
async fn network_response_chunks_cross_the_worker_dispatch_boundary() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let size = 25 * 1024 * 1024;
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0; 4096];
        assert!(socket.read(&mut request).await.unwrap() > 0);
        socket
            .write_all(
                format!("HTTP/1.1 200 OK\r\nContent-Length: {size}\r\nConnection: close\r\n\r\n")
                    .as_bytes(),
            )
            .await
            .unwrap();
        socket.write_all(&vec![42; size]).await.unwrap();
    });
    let directory = tempfile::tempdir().unwrap();
    let store = Arc::new(PackageStore::new(directory.path().join("store")).unwrap());
    let (mut common, mut versioned) = opaque_manifest("network-test", directory.path());
    common.permissions = vec![crate::package_manifest::PermissionRequest {
        capability: "network".into(),
        scopes: vec![origin.clone()],
    }];
    if let crate::package_manifest::VersionedManifest::V2(manifest) = &mut versioned {
        manifest.permissions = common.permissions.clone();
    }
    let installed = install_opaque_package(&store, directory.path(), &versioned);
    let (grant, token) =
        Grant::derive(&common, installed.hash, 41, 7, "network-test".into(), &[]).unwrap();
    let supervisor = PackageWorkerSupervisor::new(1);
    supervisor.bind_store(store);
    let key = (common.id.clone(), common.version.clone());
    supervisor.insert_failed(key.clone());
    {
        let mut workers = lock(&supervisor.inner.workers);
        let worker = workers.get_mut(&key).unwrap();
        worker.generation = 7;
        worker.health.state = WorkerState::Running;
        worker.grant = Some(grant.clone());
    }
    let broker = BrokerConfig::new([origin.clone()], vec![])
        .unwrap()
        .enable_local_test_origin();
    let reply = dispatch(
        &supervisor.inner,
        &grant,
        &broker,
        None,
        &worker_call(
            &token,
            7,
            WorkerMethod::NetworkFetch,
            serde_json::json!({"url": origin, "response_mode": "chunks"}),
        ),
    )
    .await
    .unwrap();
    server.await.unwrap();
    assert_eq!(reply["size"], size);
    let handle = reply["response_handle"].as_str().unwrap();
    let mut restored = Vec::new();
    while restored.len() < size {
        let reply = dispatch(
            &supervisor.inner,
            &grant,
            &broker,
            None,
            &worker_call(
                &token,
                7,
                WorkerMethod::NetworkFetch,
                serde_json::json!({"url": origin, "response_handle": handle,
                    "offset": restored.len(), "length": 256 * 1024}),
            ),
        )
        .await
        .unwrap();
        assert!(serde_json::to_vec(&reply).unwrap().len() < 700 * 1024);
        restored.extend(
            base64::engine::general_purpose::STANDARD
                .decode(reply["bytes"].as_str().unwrap())
                .unwrap(),
        );
    }
    assert_eq!(restored, vec![42; size]);
    assert!(dispatch(
        &supervisor.inner,
        &grant,
        &broker,
        None,
        &worker_call(
            &token,
            8,
            WorkerMethod::NetworkFetch,
            serde_json::json!({"url": origin, "response_handle": handle, "offset": 0, "length": 1})
        )
    )
    .await
    .is_err());
    dispatch(
        &supervisor.inner,
        &grant,
        &broker,
        None,
        &worker_call(
            &token,
            7,
            WorkerMethod::NetworkFetch,
            serde_json::json!({"url": origin, "response_handle": handle, "close": true}),
        ),
    )
    .await
    .unwrap();
    assert_eq!(supervisor.inner.network_responses.len(), 0);
    let owner = super::super::super::super::calls_dispatch::network_owner(&key.0, &key.1, 7);
    supervisor
        .inner
        .network_responses
        .reserve(&owner, &key.0, &origin, vec![1])
        .unwrap();
    supervisor.stop(&key.0, &key.1).await.unwrap();
    assert_eq!(supervisor.inner.network_responses.len(), 0);
}
