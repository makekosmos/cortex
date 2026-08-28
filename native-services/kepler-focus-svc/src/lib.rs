#![cfg_attr(test, allow(clippy::unwrap_used))]

//! Library facade для kepler-focus-svc — выставляет `protocol` модуль с
//! Request/Response типами и `dispatch` функцией. Unit-тесты для wire format
//! живут здесь (pipe / SCM код требует Windows runtime → не unit-testable).

pub mod protocol;

#[cfg(windows)]
pub mod ntfs_scan;

pub const SERVICE_NAME: &str = "KosmosSystemSvc";
pub const LEGACY_SERVICE_NAME: &str = "KeplerFocusSvc";

pub fn uninstall_service_names() -> [&'static str; 2] {
    [SERVICE_NAME, LEGACY_SERVICE_NAME]
}

#[cfg(test)]
mod service_name_tests {
    use super::*;

    #[test]
    fn uninstall_targets_new_and_legacy_service_names() {
        assert_eq!(
            uninstall_service_names(),
            [SERVICE_NAME, LEGACY_SERVICE_NAME]
        );
    }
}
