use super::*;
use crate::package_manifest::{IntegrationManifest, IntegrationSetting, IntegrationSettingKind};

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
