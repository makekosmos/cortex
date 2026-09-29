use super::*;
use crate::package_manifest::{
    IntegrationManifest, IntegrationSetting, IntegrationSettingKind, ManifestData, ManifestTarget,
    ManifestV2, PackageKind, TargetOs, TargetRuntime, VersionedManifest,
};
use sha2::{Digest, Sha256};
use std::{fs, io::Write, path::Path};
use tempfile::tempdir;
use zip::write::FileOptions;

fn integration_manifest(version: &str) -> VersionedManifest {
    VersionedManifest::V2(ManifestV2 {
        schema_version: 2,
        id: "com.kosmos.provider".into(),
        name: "Provider".into(),
        description: None,
        version: version.into(),
        kind: PackageKind::Source,
        engine_api: ">=1.0.0".into(),
        entrypoint: "worker.exe".into(),
        icon: None,
        publisher: "kosmos".into(),
        permissions: vec![],
        targets: vec![ManifestTarget {
            runtime: TargetRuntime::Worker,
            os: vec![TargetOs::Windows, TargetOs::Macos, TargetOs::Linux],
            arch: None,
            entrypoint: Some("worker.exe".into()),
        }],
        data: ManifestData {
            access: vec![],
            defines: vec![],
            mappings: vec![],
        },
        integration: Some(IntegrationManifest {
            settings: vec![
                IntegrationSetting {
                    key: "username".into(),
                    label: "Username".into(),
                    kind: IntegrationSettingKind::Text,
                    description: None,
                    required: false,
                    injection: None,
                },
                IntegrationSetting {
                    key: "session".into(),
                    label: "Session".into(),
                    kind: IntegrationSettingKind::Secret,
                    description: None,
                    required: false,
                    injection: None,
                },
            ],
            login: None,
            schedule: None,
        }),
    })
}

fn install_integration_package(root: &Path, service: &PackageService, version: &str) {
    let manifest = integration_manifest(version);
    let archive = root.join(format!("{version}.kspkg"));
    let mut zip = zip::ZipWriter::new(fs::File::create(&archive).unwrap());
    zip.start_file("manifest.json", FileOptions::default()).unwrap();
    zip.write_all(&serde_json::to_vec(&manifest).unwrap()).unwrap();
    zip.start_file("worker.exe", FileOptions::default()).unwrap();
    zip.write_all(b"worker").unwrap();
    zip.finish().unwrap();
    let bytes = fs::read(&archive).unwrap();
    let hash = format!("{:x}", Sha256::digest(&bytes));
    service
        .store
        .install_versioned(&archive, bytes.len() as u64, &hash, &manifest, 1)
        .unwrap();
}

#[test]
fn credential_presence_requires_one_configured_and_all_required_settings() {
    let integration = IntegrationManifest {
        settings: vec![
            IntegrationSetting {
                key: "required".into(),
                label: "Required".into(),
                kind: IntegrationSettingKind::Text,
                description: None,
                required: true,
                injection: None,
            },
            IntegrationSetting {
                key: "optional".into(),
                label: "Optional".into(),
                kind: IntegrationSettingKind::Text,
                description: None,
                required: false,
                injection: None,
            },
        ],
        login: None,
        schedule: None,
    };
    assert!(!has_integration_credential(
        &integration,
        "pkg",
        "1.0.0",
        &[("optional".into(), "value".into())].into()
    ));
    assert!(has_integration_credential(
        &integration,
        "pkg",
        "1.0.0",
        &[("required".into(), "value".into())].into()
    ));
    assert!(has_integration_credential(
        &integration,
        "pkg",
        "1.0.0",
        &[
            ("required".into(), "value".into()),
            ("optional".into(), "value".into())
        ]
        .into()
    ));
}

#[test]
fn integration_values_are_validated_before_persistence() {
    assert!(valid_integration_value("session-cookie"));
    assert!(!valid_integration_value(" leading-space"));
    assert!(!valid_integration_value("trailing-space "));
    assert!(!valid_integration_value("line\nfeed"));
}

#[tokio::test]
async fn clearing_provider_credentials_clears_and_disables_all_versions() {
    let dir = tempdir().unwrap();
    let mut service = PackageService::from_parts(dir.path().join("packages"), None, Some(dir.path().join("apps"))).unwrap();
    install_integration_package(dir.path(), &service, "1.0.0");
    install_integration_package(dir.path(), &service, "2.0.0");
    service.store.enable_worker("com.kosmos.provider", "1.0.0").unwrap();
    service.store.enable_worker("com.kosmos.provider", "2.0.0").unwrap();
    service.configure_workers(
        PackageWorkerSupervisor::new(1),
        Vec::new(),
        "test".into(),
    );

    let mut settings = PackageIntegrationSettings::default();
    settings.values.insert(
        "com.kosmos.provider@1.0.0".into(),
        [("username".into(), "old".into())].into(),
    );
    settings.values.insert(
        "com.kosmos.provider@2.0.0".into(),
        [("username".into(), "new".into())].into(),
    );
    fs::write(
        service.integration_settings_path(),
        serde_json::to_vec(&settings).unwrap(),
    )
    .unwrap();
    save_package_integration_secret("com.kosmos.provider", "1.0.0", "session", "old-secret")
        .unwrap();
    save_package_integration_secret("com.kosmos.provider", "2.0.0", "session", "new-secret")
        .unwrap();

    save_package_integration_secret("com.kosmos.provider", "1.0.0", ":huawei-refresh:session", "rotated-secret").unwrap();
    service.clear_integration_values("com.kosmos.provider").await.unwrap();
    assert!(read_package_integration_secret("com.kosmos.provider", "1.0.0", ":huawei-refresh:session").is_none());

    assert!(service.read_integration_settings().values.is_empty());
    assert!(read_package_integration_secret("com.kosmos.provider", "1.0.0", "session").is_none());
    assert!(read_package_integration_secret("com.kosmos.provider", "2.0.0", "session").is_none());
    assert!(service
        .store
        .list()
        .unwrap()
        .into_iter()
        .all(|package| !package.enabled));
}

#[test]
fn uninstall_clears_only_the_exact_integration_version() {
    let dir = tempdir().unwrap();
    let service = PackageService::from_parts(dir.path().join("packages"), None, Some(dir.path().join("apps"))).unwrap();
    install_integration_package(dir.path(), &service, "3.0.0");
    install_integration_package(dir.path(), &service, "4.0.0");

    let mut settings = PackageIntegrationSettings::default();
    settings.values.insert(
        "com.kosmos.provider@3.0.0".into(),
        [("username".into(), "old".into())].into(),
    );
    settings.values.insert(
        "com.kosmos.provider@4.0.0".into(),
        [("username".into(), "new".into())].into(),
    );
    fs::write(
        service.integration_settings_path(),
        serde_json::to_vec(&settings).unwrap(),
    )
    .unwrap();
    save_package_integration_secret("com.kosmos.provider", "3.0.0", "session", "old-secret")
        .unwrap();
    save_package_integration_secret("com.kosmos.provider", "4.0.0", "session", "new-secret")
        .unwrap();

    service.uninstall("com.kosmos.provider", "3.0.0").unwrap();

    assert!(service.store.installed("com.kosmos.provider", "3.0.0").is_err());
    assert!(service.store.installed("com.kosmos.provider", "4.0.0").is_ok());
    assert!(!service
        .read_integration_settings()
        .values
        .contains_key("com.kosmos.provider@3.0.0"));
    assert_eq!(
        service
            .read_integration_settings()
            .values
            .get("com.kosmos.provider@4.0.0")
            .and_then(|values| values.get("username"))
            .map(String::as_str),
        Some("new")
    );
    assert!(read_package_integration_secret("com.kosmos.provider", "3.0.0", "session").is_none());
    assert_eq!(
        read_package_integration_secret("com.kosmos.provider", "4.0.0", "session").as_deref(),
        Some("new-secret")
    );
}
