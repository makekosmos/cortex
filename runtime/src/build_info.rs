// One product, one version (KOS-233): the Engine no longer carries its own
// `0.1.x` release line. `desktop/scripts/release-build-env.mjs` injects the
// Mundus Desktop product version (`desktop/release-versions.json`) and the
// source commit it was built from as env vars at compile time; `option_env!`
// bakes them into the binary. A dev build (`cargo build` outside the desktop
// build scripts) has neither set. The raw fields stay empty so callers can
// tell a dev build from a release. The string a person sees is `dev`: the
// crate version 0.1.0 is the Cargo placeholder, not a product version.
//
// This is distinct from `protocol_version::API_VERSION`, which is the
// Engine↔shell wire contract and does not move with the product version.

const PRODUCT_VERSION: Option<&str> = option_env!("MUNDUS_PRODUCT_VERSION");
const ENGINE_SOURCE_COMMIT: Option<&str> = option_env!("MUNDUS_ENGINE_SOURCE_COMMIT");

/// Product version this Engine binary was built as part of (e.g. `"0.9.39"`),
/// or `""` for a build that did not go through `build-backend.mjs`.
pub fn engine_version() -> &'static str {
    PRODUCT_VERSION.unwrap_or("")
}

/// 40-character git commit this Engine binary was built from, or `""` for a
/// build that did not go through `build-backend.mjs`.
pub fn engine_source_commit() -> &'static str {
    ENGINE_SOURCE_COMMIT.unwrap_or("")
}

/// Product-facing version string: the injected Desktop version when available,
/// otherwise `dev` (a bare `cargo build`).
pub fn display_version() -> &'static str {
    crate::build_metadata::compiled().version
}

/// Explicit channel: an unversioned Cargo build is always dev, even optimized.
pub fn channel() -> &'static str {
    crate::build_metadata::compiled().channel
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falls_back_to_empty_string_without_injected_env() {
        // This crate's own test build never sets MUNDUS_PRODUCT_VERSION /
        // MUNDUS_ENGINE_SOURCE_COMMIT, so the fallback path is what actually
        // runs here — assert it stays a valid (empty) string, never panics.
        assert!(engine_version().is_empty());
        assert!(engine_source_commit().is_empty());
    }

    #[test]
    fn display_version_is_dev_when_the_product_version_is_not_injected() {
        // This test build does not set MUNDUS_PRODUCT_VERSION. 0.1.0 would
        // be the crate placeholder, which a release once shipped by mistake.
        assert_eq!(display_version(), "dev");
    }
}
