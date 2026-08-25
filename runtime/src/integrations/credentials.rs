#[cfg(not(test))]
use super::config::KEYRING_SERVICE;
use super::*;

#[cfg(not(test))]
pub(crate) fn credential_entry(provider: Provider) -> Result<keyring::Entry, String> {
    let service = if std::env::var("KOSMOS_TEST_MODE").as_deref() == Ok("1") {
        "kosmos-kepler-test"
    } else {
        KEYRING_SERVICE
    };
    keyring::Entry::new(service, provider.keyring_user())
        .map_err(|error| format!("Windows Credential Manager: {error}"))
}

#[cfg(not(test))]
pub(crate) fn read_credential(provider: Provider) -> Option<String> {
    credential_entry(provider).ok()?.get_password().ok()
}

#[cfg(test)]
pub(crate) fn read_credential(_provider: Provider) -> Option<String> {
    None
}

#[cfg(not(test))]
pub(crate) fn save_credential(provider: Provider, secret: &str) -> Result<(), String> {
    credential_entry(provider)?
        .set_password(secret)
        .map_err(|error| format!("РќРµ СѓРґР°Р»РѕСЃСЊ СЃРѕС…СЂР°РЅРёС‚СЊ РєР»СЋС‡: {error}"))
}

#[cfg(test)]
pub(crate) fn save_credential(_provider: Provider, _secret: &str) -> Result<(), String> {
    Ok(())
}

#[cfg(not(test))]
pub(crate) fn delete_credential(provider: Provider) -> Result<(), String> {
    match credential_entry(provider)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(format!(
            "РќРµ СѓРґР°Р»РѕСЃСЊ СѓРґР°Р»РёС‚СЊ РєР»СЋС‡: {error}"
        )),
    }
}

#[cfg(test)]
pub(crate) fn delete_credential(_provider: Provider) -> Result<(), String> {
    Ok(())
}

fn provider_snapshot(provider: Provider, settings: &ProviderSettings) -> Value {
    json!({
        "id": provider.id(),
        "label": provider.label(),
        "credentialLabel": provider.credential_label(),
        "credentialUrl": provider.credential_url(),
        "hasCredential": read_credential(provider).is_some(),
        "settings": settings,
    })
}

pub(crate) fn snapshot(config: &IntegrationsConfig) -> Value {
    json!({
        "providers": Provider::ALL
            .into_iter()
            .map(|provider| provider_snapshot(provider, config.provider(provider)))
            .collect::<Vec<_>>(),
        "bodyWeightKg": config.body_weight_kg,
    })
}

pub(crate) fn provider_from_params(params: &Value) -> Result<Provider, String> {
    Provider::parse(
        params
            .get("provider")
            .and_then(Value::as_str)
            .ok_or("РќРµ СѓРєР°Р·Р°РЅР° РёРЅС‚РµРіСЂР°С†РёСЏ")?,
    )
}

pub(crate) fn validate_interval(value: u64) -> Result<u64, String> {
    ALLOWED_INTERVALS
        .contains(&value)
        .then_some(value)
        .ok_or_else(|| {
            "РќРµРґРѕРїСѓСЃС‚РёРјР°СЏ С‡Р°СЃС‚РѕС‚Р° СЃРёРЅС…СЂРѕРЅРёР·Р°С†РёРё".to_string()
        })
}
