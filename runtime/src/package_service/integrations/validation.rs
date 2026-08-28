use super::*;

pub(super) fn valid_integration_value(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 4096
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

pub(super) fn has_integration_credential(
    integration: &IntegrationManifest,
    id: &str,
    version: &str,
    values: &HashMap<String, String>,
) -> bool {
    let configured = integration.settings.iter().any(|setting| match setting.kind {
        crate::package_manifest::IntegrationSettingKind::Text => values.contains_key(&setting.key),
        crate::package_manifest::IntegrationSettingKind::Secret => {
            read_package_integration_secret(id, version, &setting.key).is_some()
        }
    });
    configured
        && integration.settings.iter().filter(|setting| setting.required).all(|setting| {
            match setting.kind {
                crate::package_manifest::IntegrationSettingKind::Text => {
                    values.contains_key(&setting.key)
                }
                crate::package_manifest::IntegrationSettingKind::Secret => {
                    read_package_integration_secret(id, version, &setting.key).is_some()
                }
            }
        })
}
