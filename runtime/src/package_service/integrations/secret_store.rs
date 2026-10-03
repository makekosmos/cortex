use super::PackageError;

#[cfg(not(any(test, feature = "package-worker-fixture")))]
fn package_integration_entry(id: &str, version: &str, setting: &str) -> Option<keyring::Entry> {
    keyring::Entry::new(
        crate::brand::KEYRING_SERVICE,
        &format!("package-integration:{id}:{version}:{setting}"),
    )
    .ok()
}

#[cfg(not(any(test, feature = "package-worker-fixture")))]
pub(crate) fn read_package_integration_secret(
    id: &str,
    version: &str,
    setting: &str,
) -> Option<String> {
    package_integration_entry(id, version, setting)?.get_password().ok()
}

#[cfg(not(any(test, feature = "package-worker-fixture")))]
pub(crate) fn save_package_integration_secret(
    id: &str,
    version: &str,
    setting: &str,
    value: &str,
) -> Result<(), PackageError> {
    package_integration_entry(id, version, setting)
        .ok_or(PackageError::Persistence)?
        .set_password(value)
        .map_err(|_| PackageError::Persistence)
}

#[cfg(not(any(test, feature = "package-worker-fixture")))]
pub(crate) fn clear_package_integration_secret(
    id: &str,
    version: &str,
    setting: &str,
) -> Result<(), PackageError> {
    match package_integration_entry(id, version, setting)
        .ok_or(PackageError::Persistence)?
        .delete_credential()
    {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err(PackageError::Persistence),
    }
}

#[cfg(any(test, feature = "package-worker-fixture"))]
mod test_store {
    use super::*;
    use std::{collections::HashMap, sync::{Mutex, OnceLock}};

    static VALUES: OnceLock<Mutex<HashMap<(String, String, String), String>>> = OnceLock::new();

    fn values() -> &'static Mutex<HashMap<(String, String, String), String>> {
        VALUES.get_or_init(|| Mutex::new(HashMap::new()))
    }

    pub(super) fn read(id: &str, version: &str, setting: &str) -> Option<String> {
        values()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .get(&(id.into(), version.into(), setting.into()))
            .cloned()
    }

    pub(super) fn save(
        id: &str,
        version: &str,
        setting: &str,
        value: &str,
    ) -> Result<(), PackageError> {
        values()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .insert((id.into(), version.into(), setting.into()), value.into());
        Ok(())
    }

    pub(super) fn clear(id: &str, version: &str, setting: &str) -> Result<(), PackageError> {
        values()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .remove(&(id.into(), version.into(), setting.into()));
        Ok(())
    }
}

#[cfg(any(test, feature = "package-worker-fixture"))]
pub(crate) fn read_package_integration_secret(
    id: &str,
    version: &str,
    setting: &str,
) -> Option<String> {
    test_store::read(id, version, setting)
}

#[cfg(any(test, feature = "package-worker-fixture"))]
pub(crate) fn save_package_integration_secret(
    id: &str,
    version: &str,
    setting: &str,
    value: &str,
) -> Result<(), PackageError> {
    test_store::save(id, version, setting, value)
}

#[cfg(any(test, feature = "package-worker-fixture"))]
pub(crate) fn clear_package_integration_secret(
    id: &str,
    version: &str,
    setting: &str,
) -> Result<(), PackageError> {
    test_store::clear(id, version, setting)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secret_store_round_trip() {
        save_package_integration_secret("pkg", "1.0.0", "session", "opaque-cookie").unwrap();
        assert_eq!(
            read_package_integration_secret("pkg", "1.0.0", "session").as_deref(),
            Some("opaque-cookie")
        );
        clear_package_integration_secret("pkg", "1.0.0", "session").unwrap();
        assert!(read_package_integration_secret("pkg", "1.0.0", "session").is_none());
    }
}
