//! Version parsing/compare for the self-updater, built on the `semver` crate
//! already used elsewhere in this crate for version matching (see
//! `package_manifest.rs`, `runtime_grants.rs`, `engine_api/handlers/
//! authorization.rs`) rather than a hand-rolled comparator.
use semver::Version;

/// Running Engine version, compared against the update feed to decide
/// whether a newer build is available. KOS-233 will make this equal the
/// Mundus Desktop product version via a build-time env var; until that
/// lands this falls back to the crate version. This function is the single
/// call site to flip when KOS-233 ships.
pub(crate) fn current_version() -> String {
    option_env!("MUNDUS_PRODUCT_VERSION")
        .unwrap_or(env!("CARGO_PKG_VERSION"))
        .to_string()
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
    fn current_version_uses_product_version_with_crate_fallback() {
        assert_eq!(
            current_version(),
            option_env!("MUNDUS_PRODUCT_VERSION").unwrap_or(env!("CARGO_PKG_VERSION"))
        );
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
