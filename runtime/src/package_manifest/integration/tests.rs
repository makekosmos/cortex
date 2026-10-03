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
                code_exchange: None,
            }),
            schedule: Some(IntegrationSchedule {
                interval_seconds: 3600,
            }),
        }),
        store: None,
    }
}

#[test]
fn validates_login_and_schedule() {
    assert!(manifest().validate().is_ok());
}

#[test]
fn huawei_login_restricts_exchange_and_session_destinations() {
    let mut value = manifest();
    value.permissions[0].scopes = vec!["https://sportdata-dre.things.dbankcloud.com/".into()];
    let integration = value.integration.as_mut().unwrap();
    integration.settings[0].injection = Some(
        serde_json::from_value(serde_json::json!({
            "kind":"json", "origins":["https://sportdata-dre.things.dbankcloud.com/"],
            "body_fields":{"token":"accessToken"}, "header_fields":{"x-huid":"uid"}
        }))
        .unwrap(),
    );
    let login = integration.login.as_mut().unwrap();
    login.start_url = "https://oauth-login.cloud.huawei.com/oauth2/v3/authorize".into();
    login.completion_url = "hms://redirect_url".into();
    login.allowed_cookie_names.clear();
    login.code_exchange = Some("huawei_health".into());
    assert!(value.validate().is_ok());
    let mut wrong = value.clone();
    wrong
        .integration
        .as_mut()
        .unwrap()
        .login
        .as_mut()
        .unwrap()
        .completion_url = "hms://other".into();
    assert!(wrong.validate().is_err());
    if let Some(SecretInjection::Json { body_fields, .. }) =
        &mut value.integration.as_mut().unwrap().settings[0].injection
    {
        body_fields.insert("refresh".into(), "refreshToken".into());
    }
    assert!(value.validate().is_err());
}

#[test]
fn json_session_manifest_validates_mappings_and_scope() {
    let mut value = manifest();
    let integration = value.integration.as_mut().unwrap();
    integration.login = None;
    integration.settings[0].injection = Some(
        serde_json::from_value(serde_json::json!({
            "kind": "json", "origins": ["https://example.com/"],
            "body_fields": {"token": "accessToken"},
            "header_fields": {"x-huid": "uid"}, "headers": {"x-version": "test"}
        }))
        .unwrap(),
    );
    assert!(value.validate().is_ok());
    for header in ["Host", "Authorization", "Cookie", "Content-Length"] {
        let mut invalid = value.clone();
        if let Some(SecretInjection::Json { header_fields, .. }) =
            &mut invalid.integration.as_mut().unwrap().settings[0].injection
        {
            header_fields.insert(header.into(), "uid".into());
        }
        assert!(invalid.validate().is_err());
    }
    value.permissions[0].scopes = vec!["https://other.example/".into()];
    assert!(value.validate().is_err());
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
