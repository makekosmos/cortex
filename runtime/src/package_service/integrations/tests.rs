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
    integration_manifest_with(version, IntegrationSettingKind::Text, false)
}

fn integration_manifest_with(
    version: &str,
    username_kind: IntegrationSettingKind,
    // A required-but-unset secret keeps `has_integration_credential` false so
    // `set_integration_value` stays below the worker-enable path in tests.
    session_required: bool,
) -> VersionedManifest {
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
                    kind: username_kind,
                    description: None,
                    required: false,
                    injection: None,
                },
                IntegrationSetting {
                    key: "session".into(),
                    label: "Session".into(),
                    kind: IntegrationSettingKind::Secret,
                    description: None,
                    required: session_required,
                    injection: None,
                },
            ],
            login: None,
            schedule: None,
        }),
    })
}

fn install_integration_package(root: &Path, service: &PackageService, version: &str) {
    install_integration_manifest(root, service, integration_manifest(version));
}

fn install_integration_manifest(
    root: &Path,
    service: &PackageService,
    manifest: VersionedManifest,
) {
    let version = manifest.version().to_owned();
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
    let mut service = PackageService::from_parts(
        dir.path().join("packages"),
        None,
        Some(dir.path().join("apps")),
    ).unwrap();
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

    save_package_integration_secret(
        "com.kosmos.provider",
        "1.0.0",
        ":huawei-refresh:session",
        "rotated-secret",
    ).unwrap();
    service.clear_integration_values("com.kosmos.provider").await.unwrap();
    assert!(read_package_integration_secret(
        "com.kosmos.provider",
        "1.0.0",
        ":huawei-refresh:session",
    ).is_none());

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
    let service = PackageService::from_parts(
        dir.path().join("packages"),
        None,
        Some(dir.path().join("apps")),
    ).unwrap();
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

fn configured_service(dir: &tempfile::TempDir) -> PackageService {
    let mut service =
        PackageService::from_parts(dir.path().join("packages"), None, Some(dir.path().join("apps")))
            .unwrap();
    service.configure_workers(
        PackageWorkerSupervisor::new(1),
        Vec::new(),
        "test".into(),
    );
    service
}

/// KOS-279: a username is public data — it must land in
/// integration-settings.json, never in the credential vault.
#[tokio::test]
async fn username_kind_value_is_stored_as_config_not_vault() {
    let dir = tempdir().unwrap();
    let service = configured_service(&dir);
    install_integration_manifest(
        dir.path(),
        &service,
        integration_manifest_with("1.0.0", IntegrationSettingKind::Username, true),
    );

    service
        .set_integration_value(
            "com.kosmos.provider",
            Some("username"),
            Some("username"),
            "public-nick",
        )
        .await
        .unwrap();

    assert_eq!(
        service
            .read_integration_settings()
            .values
            .get("com.kosmos.provider@1.0.0")
            .and_then(|values| values.get("username"))
            .map(String::as_str),
        Some("public-nick")
    );
    assert!(
        read_package_integration_secret("com.kosmos.provider", "1.0.0", "username").is_none(),
        "a public username must not enter the credential vault"
    );
}

/// KOS-279: `set_credential` echoes the kind the UI rendered; the manifest
/// kind is authoritative, so a mismatched kind is rejected.
#[tokio::test]
async fn set_integration_value_rejects_a_kind_mismatch() {
    let dir = tempdir().unwrap();
    let service = configured_service(&dir);
    install_integration_manifest(
        dir.path(),
        &service,
        integration_manifest_with("1.0.0", IntegrationSettingKind::Username, true),
    );

    assert!(service
        .set_integration_value(
            "com.kosmos.provider",
            Some("username"),
            Some("api_key"),
            "public-nick",
        )
        .await
        .is_err());
    assert!(!service
        .read_integration_settings()
        .values
        .contains_key("com.kosmos.provider@1.0.0"));
}

/// KOS-279: a value vaulted while an older manifest declared the setting
/// `secret` is moved to plain config and deleted from the vault once the
/// manifest marks the setting public (`username`/`text`).
#[tokio::test]
async fn vaulted_username_moves_to_config_after_manifest_relabels_it() {
    let dir = tempdir().unwrap();
    let service = configured_service(&dir);
    install_integration_manifest(
        dir.path(),
        &service,
        integration_manifest_with("1.0.0", IntegrationSettingKind::Username, false),
    );
    // Simulate the old manifest generation that kept the username in the
    // credential vault.
    save_package_integration_secret("com.kosmos.provider", "1.0.0", "username", "old-nick")
        .unwrap();

    let providers = service.integration_provider_snapshots().unwrap();

    assert!(
        read_package_integration_secret("com.kosmos.provider", "1.0.0", "username").is_none(),
        "the migrated value must leave the vault"
    );
    let provider = providers
        .iter()
        .find(|p| p.get("id").and_then(Value::as_str) == Some("com.kosmos.provider"))
        .expect("provider snapshot");
    assert_eq!(
        provider
            .get("settingValues")
            .and_then(|v| v.get("username"))
            .and_then(Value::as_str),
        Some("old-nick")
    );
    // The migrated config value counts towards `hasCredential`.
    assert_eq!(
        provider.get("hasCredential").and_then(Value::as_bool),
        Some(true)
    );
}
