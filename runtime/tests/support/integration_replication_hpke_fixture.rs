use super::*;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use ed25519_dalek::{Signer, SigningKey};
use kepler_backend::{
    package_manifest::{
        IntegrationManifest, IntegrationSetting, IntegrationSettingKind, ManifestData,
        ManifestTarget, ManifestV2, PackageKind, PermissionRequest, SecretInjection, TargetOs,
        TargetRuntime, VersionedManifest,
    },
    package_trust::{CatalogDocument, CatalogEntry, DetachedSignature, SignatureSet, TrustedKey},
};
use sha2::{Digest, Sha256};
use std::io::Write;
use zip::write::FileOptions;

pub(crate) struct Cleanup {
    _lock: std::sync::MutexGuard<'static, ()>,
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = keyring::Entry::new(
            "kosmos-kepler",
            &format!("package-integration:{PACKAGE_ID}:{PACKAGE_VERSION}:{SETTING}"),
        )
        .and_then(|entry| entry.delete_credential());
        for node in ["origin-node", "recipient-node", "foreign-node"] {
            let _ = kepler_backend::package_service::credential_envelope::clear_identity(node);
        }
        unsafe {
            std::env::remove_var("KOSMOS_FAKE_PROVIDER_RESULT_MARKER");
            std::env::remove_var("KOSMOS_FIXTURE_ENTRY_MARKER");
            std::env::remove_var("KOSMOS_FIXTURE_BOOTSTRAP_MARKER");
        }
    }
}

pub fn cleanup(marker: &Path) -> Cleanup {
    let lock = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
    unsafe {
        std::env::set_var("KOSMOS_FAKE_PROVIDER_RESULT_MARKER", marker);
        std::env::set_var(
            "KOSMOS_FIXTURE_ENTRY_MARKER",
            marker.with_extension("entry"),
        );
        std::env::set_var(
            "KOSMOS_FIXTURE_BOOTSTRAP_MARKER",
            marker.with_extension("bootstrap"),
        );
    }
    Cleanup { _lock: lock }
}

pub fn manifest(origin: &str) -> VersionedManifest {
    VersionedManifest::V2(ManifestV2 {
        schema_version: 2,
        id: PACKAGE_ID.into(),
        name: "HPKE replication fixture".into(),
        description: Some("test-only signed source provider".into()),
        version: PACKAGE_VERSION.into(),
        kind: PackageKind::Source,
        engine_api: ">=1.0.0".into(),
        entrypoint: "package-worker-fixture.exe".into(),
        icon: None,
        publisher: "kosmos".into(),
        permissions: vec![PermissionRequest {
            capability: "network".into(),
            scopes: vec![origin.into()],
        }],
        targets: vec![ManifestTarget {
            runtime: TargetRuntime::Worker,
            os: vec![TargetOs::Windows],
            arch: None,
            entrypoint: Some("package-worker-fixture.exe".into()),
        }],
        data: ManifestData {
            access: vec![],
            defines: vec![],
            mappings: vec![],
        },
        integration: Some(IntegrationManifest {
            settings: vec![
                IntegrationSetting {
                    key: "endpoint".into(),
                    label: "Provider endpoint".into(),
                    kind: IntegrationSettingKind::Text,
                    description: None,
                    required: true,
                    injection: None,
                },
                IntegrationSetting {
                    key: SETTING.into(),
                    label: "Session".into(),
                    kind: IntegrationSettingKind::Secret,
                    description: None,
                    required: true,
                    injection: Some(SecretInjection::Header {
                        origins: vec![origin.into()],
                        name: "Authorization".into(),
                        prefix: "Bearer ".into(),
                    }),
                },
            ],
            login: None,
            schedule: None,
        }),
    })
}

pub fn archive(dir: &Path, versioned: &VersionedManifest) -> (std::path::PathBuf, u64, String) {
    let path = dir.join("hpke-replication.kspkg");
    let file = std::fs::File::create(&path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    zip.start_file("manifest.json", FileOptions::default())
        .unwrap();
    zip.write_all(&serde_json::to_vec(versioned).unwrap())
        .unwrap();
    zip.start_file("package-worker-fixture.exe", FileOptions::default())
        .unwrap();
    zip.write_all(&std::fs::read(env!("CARGO_BIN_EXE_package-worker-fixture")).unwrap())
        .unwrap();
    zip.finish().unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let hash = format!("{:x}", Sha256::digest(&bytes));
    (path, bytes.len() as u64, hash)
}

pub fn signed_catalog(
    origin: &str,
    size: u64,
    hash: &str,
) -> (Vec<u8>, SignatureSet, TrustedKey, TrustedKey) {
    let signing = SigningKey::from_bytes(&[77; 32]);
    let release = TrustedKey {
        key_id: "kosmos-test-release".into(),
        public_key: STANDARD.encode(signing.verifying_key().as_bytes()),
    };
    let root_signing = SigningKey::from_bytes(&[78; 32]);
    let root = TrustedKey {
        key_id: "kosmos-test-root".into(),
        public_key: STANDARD.encode(root_signing.verifying_key().as_bytes()),
    };
    let document = CatalogDocument {
        schema_version: 1,
        sequence: 1,
        issued_at: "2026-09-07T00:00:00Z".into(),
        expires_at: "2027-09-07T00:00:00Z".into(),
        packages: vec![CatalogEntry {
            manifest: manifest(origin),
            archive_url: "https://test.invalid/hpke-replication.kspkg".into(),
            sha256: hash.into(),
            size,
        }],
    };
    let bytes = serde_json::to_vec(&document).unwrap();
    let signatures = SignatureSet {
        schema_version: 1,
        signatures: vec![DetachedSignature {
            key_id: release.key_id.clone(),
            algorithm: "ed25519".into(),
            signature: STANDARD.encode(signing.sign(&bytes).to_bytes()),
        }],
    };
    (bytes, signatures, root, release)
}

pub async fn start_sync(host: &ArkHost, device: &str, port: u16, ticket: Option<&str>) {
    let response = host
        .request(
            "start_sync",
            json!({
                "space_id": SPACE_ID,
                "device_id": device,
                "device_name": device,
                "port": port,
                "auth_secret": "hpke-replication-auth",
                "use_iroh": true,
                "iroh_peer_ticket": ticket,
                "discovery_enabled": false,
            }),
        )
        .await
        .unwrap();
    assert!(response.ok, "start_sync failed: {:?}", response.error);
}

pub fn free_port() -> u16 {
    std::net::TcpListener::bind(("127.0.0.1", 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

pub async fn start_hosts(
    setup: &support::IntegrationReplicationSetup,
    core_binary: &Path,
) -> (ArkHost, ArkHost) {
    let origin_host = ArkHost::spawn(core_binary, setup.origin_db.to_str().unwrap())
        .await
        .unwrap();
    let recipient_host = ArkHost::spawn(core_binary, setup.recipient_db.to_str().unwrap())
        .await
        .unwrap();
    start_sync(&origin_host, &setup.origin.node_id, free_port(), None).await;
    let origin_ticket = origin_host
        .request("get_own_iroh_ticket", json!({}))
        .await
        .unwrap()
        .data
        .as_str()
        .unwrap()
        .to_owned();
    start_sync(
        &recipient_host,
        &setup.recipient.node_id,
        free_port(),
        Some(&origin_ticket),
    )
    .await;
    let recipient_ticket = recipient_host
        .request("get_own_iroh_ticket", json!({}))
        .await
        .unwrap()
        .data
        .as_str()
        .unwrap()
        .to_owned();
    start_sync(
        &origin_host,
        &setup.origin.node_id,
        free_port(),
        Some(&recipient_ticket),
    )
    .await;
    let restarted_origin_ticket = origin_host
        .request("get_own_iroh_ticket", json!({}))
        .await
        .unwrap()
        .data
        .as_str()
        .unwrap()
        .to_owned();
    start_sync(
        &recipient_host,
        &setup.recipient.node_id,
        free_port(),
        Some(&restarted_origin_ticket),
    )
    .await;
    let restarted_recipient_ticket = recipient_host
        .request("get_own_iroh_ticket", json!({}))
        .await
        .unwrap()
        .data
        .as_str()
        .unwrap()
        .to_owned();
    support::refresh_transport_public_keys_named(
        setup,
        &restarted_origin_ticket,
        &restarted_recipient_ticket,
        PACKAGE_ID,
    )
    .unwrap();
    offline::wait_for_peer(&recipient_host, &setup.origin.node_id).await;
    (origin_host, recipient_host)
}
