#![cfg_attr(test, allow(clippy::unwrap_used))]

//! Library facade для focus-svc — выставляет `protocol` модуль с
//! Request/Response типами и `dispatch` функцией. Unit-тесты для wire format
//! живут здесь (pipe / SCM код требует Windows runtime → не unit-testable).

pub mod protocol;
pub mod request_io;

#[cfg(windows)]
pub mod ntfs_scan;

pub const SERVICE_NAME: &str = "MundusSystemSvc";

/// Service names registered by previous product generations. Both may exist
/// in the SCM database on upgraded machines; `uninstall` removes all of them
/// and `install`/`open` tolerate either being the active registration.
// MIGRATION(KOS-267): remove after 2026-11-01.
pub const LEGACY_SERVICE_NAMES: [&str; 2] = [
    "KosmosSystemSvc", // MIGRATION(KOS-267)
    "KeplerFocusSvc",  // MIGRATION(KOS-267)
];

pub fn service_names() -> impl Iterator<Item = &'static str> {
    std::iter::once(SERVICE_NAME).chain(LEGACY_SERVICE_NAMES)
}

pub fn uninstall_service_names() -> impl Iterator<Item = &'static str> {
    service_names()
}

#[cfg(test)]
mod service_name_tests {
    use super::*;

    #[test]
    fn uninstall_targets_new_and_legacy_service_names() {
        let names: Vec<_> = uninstall_service_names().collect();
        assert_eq!(names[0], SERVICE_NAME);
        assert!(names.contains(&"KosmosSystemSvc")); // MIGRATION(KOS-267)
        assert!(names.contains(&"KeplerFocusSvc")); // MIGRATION(KOS-267)
    }
}
