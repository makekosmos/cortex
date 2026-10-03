//! Version parsing/compare for the self-updater, built on the `semver` crate
//! already used elsewhere in this crate for version matching (see
//! `package_manifest.rs`, `runtime_grants.rs`, `engine_api/handlers/
//! authorization.rs`) rather than a hand-rolled comparator.
use semver::Version;

/// Running Engine version, compared against the update feed to decide
/// whether a newer build is available. It is the Mundus product version
/// that `build-backend.mjs` bakes in (`build_info`, KOS-233), the same
/// value the About page shows. Reading any other source made a released
/// 0.10.0 Engine report the crate's 0.1.0 and offer 0.10.0 as an update
/// forever (KOS-278).
pub(crate) fn current_version() -> String {
    crate::build_info::display_version().to_string()
}

/// True when `candidate` is strictly newer than `current` — the updater's
/// only comparison, since we never downgrade. Either side failing to parse
/// as semver (the feed is trusted GitHub-releases infrastructure we publish
/// ourselves, but a malformed value should degrade safely) is treated as
/// "not newer" rather than propagating a parse error into the update flow.
pub(crate) fn is_newer(candidate: &str, current: &str) -> bool {
    match (
        Version::parse(candidate.trim()),
        Version::parse(current.trim()),
    ) {
        (Ok(candidate), Ok(current)) => candidate > current,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_version_is_the_product_version_the_build_injects() {
        assert_eq!(current_version(), crate::build_info::display_version());
    }

    #[test]
    fn dev_display_suffix_never_enters_semver_and_real_release_comparison() {
        assert!(Version::parse(&current_version()).is_ok());
        assert!(is_newer("0.10.3", "0.10.2"));
        assert!(!is_newer("0.10.3", "0.10.3"));
    }

    #[test]
    fn strictly_greater_numeric_versions_are_newer() {
        assert!(is_newer("0.5.4", "0.5.3"));
        assert!(is_newer("0.6.0", "0.5.9"));
        assert!(is_newer("1.0.0", "0.9.9"));
    }

    #[test]
    fn equal_or_older_versions_are_never_newer() {
        assert!(!is_newer("0.5.3", "0.5.3"));
        assert!(!is_newer("0.5.2", "0.5.3"));
        assert!(!is_newer("0.4.9", "0.5.0"));
    }

    #[test]
    fn finished_release_outranks_its_own_prerelease() {
        assert!(is_newer("0.5.3", "0.5.3-beta.1"));
        assert!(!is_newer("0.5.3-beta.1", "0.5.3"));
    }

    #[test]
    fn unparseable_versions_never_report_newer() {
        assert!(!is_newer("not-a-version", "0.5.3"));
        assert!(!is_newer("0.5.3", "also-not-a-version"));
    }
}
