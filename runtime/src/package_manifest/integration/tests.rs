use super::*;
use crate::package_manifest::{
    ManifestData, ManifestTarget, PackageManifest, PermissionRequest, TargetOs, TargetRuntime,
};

fn manifest() -> ManifestV2 {
    ManifestV2 {
        schema_version: 2,
        id: "com.kosmos.source".into(),
        name: "Source".into(),
        description: None,
        version: "1.0.0".into(),
        kind: PackageKind::Source,
        engine_api: "*".into(),
        entrypoint: "worker.exe".into(),
        icon: None,
        publisher: "kosmos".into(),
        permissions: vec![PermissionRequest {
            capability: "network".into(),
            scopes: vec!["https://example.com/".into()],
        }],
        targets: vec![ManifestTarget {
            runtime: TargetRuntime::Worker,
            os: vec![TargetOs::Windows],
            arch: None,
            entrypoint: Some("worker.exe".into()),
        }],
        data: ManifestData {
            access: vec![],
            defines: vec![],
            mappings: vec![],
        },
        integration: Some(IntegrationManifest {
            settings: vec![IntegrationSetting {
                key: "session".into(),
                label: "Session".into(),
                kind: IntegrationSettingKind::Secret,
                description: None,
                required: true,
                injection: Some(SecretInjection::Cookies {
                    origins: vec!["https://example.com/".into()],
                    method: IntegrationRequestMethod::Get,
                    headers: Default::default(),
                    header_from_cookie: None,
                }),
            }],
            login: Some(BrowserLogin {
                start_url: "https://example.com/login".into(),
                completion_url: "https://example.com/account".into(),
                allowed_cookie_names: vec!["session".into()],
                secret_setting: "session".into(),
            }),
            schedule: Some(IntegrationSchedule {
                interval_seconds: 3600,
            }),
        }),
    }
}

#[test]
fn validates_login_and_schedule() {
    assert!(manifest().validate().is_ok());
}

#[test]
fn rejects_duplicate_cookies_and_unscoped_login() {
    let mut invalid = manifest();
    let login = invalid
        .integration
        .as_mut()
        .unwrap()
        .login
        .as_mut()
        .unwrap();
    login.allowed_cookie_names.push("session".into());
    assert!(invalid.validate().is_err());

    let mut invalid = manifest();
    invalid.permissions[0].scopes = vec!["https://other.example/".into()];
    assert!(invalid.validate().is_err());
}

#[test]
fn rejects_duplicate_provider_metadata() {
    let value = serde_json::json!({
        "schema_version": 2,
        "id": "com.kosmos.source",
        "name": "Source",
        "version": "1.0.0",
        "kind": "source",
        "engine_api": "*",
        "entrypoint": "worker.exe",
        "publisher": "kosmos",
        "targets": [{"runtime": "worker", "os": ["windows"]}],
        "data": {"access": [], "defines": [], "mappings": []},
        "integration": {"provider": {"id": "other"}}
    });
    assert!(PackageManifest::parse(&value.to_string()).is_err());
}
