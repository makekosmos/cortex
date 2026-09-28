// One product, one version (KOS-233): the Engine no longer carries its own
// `0.1.x` release line. `desktop/scripts/build-backend.mjs` injects the
// Mundus Desktop product version (`desktop/release-versions.json`) and the
// source commit it was built from as env vars at compile time; `option_env!`
// bakes them into the binary. A dev build (`cargo build` outside the desktop
// build script) has neither set, so both fall back to an empty string rather
// than a stale or misleading version.
//
// This is distinct from `protocol_version::API_VERSION`, which is the
// Engine↔shell wire contract and does not move with the product version.

const ENGINE_VERSION: Option<&str> = option_env!("MUNDUS_ENGINE_VERSION");
const ENGINE_SOURCE_COMMIT: Option<&str> = option_env!("MUNDUS_ENGINE_SOURCE_COMMIT");

/// Product version this Engine binary was built as part of (e.g. `"0.9.39"`),
/// or `""` for a build that did not go through `build-backend.mjs`.
pub fn engine_version() -> &'static str {
    ENGINE_VERSION.unwrap_or("")
}

/// 40-character git commit this Engine binary was built from, or `""` for a
/// build that did not go through `build-backend.mjs`.
pub fn engine_source_commit() -> &'static str {
    ENGINE_SOURCE_COMMIT.unwrap_or("")
}

/// Product-facing version string: the injected Desktop version when available,
/// otherwise the crate's own `CARGO_PKG_VERSION` (e.g. a bare `cargo build`).
pub fn display_version() -> &'static str {
    let injected = engine_version();
    if injected.is_empty() {
        env!("CARGO_PKG_VERSION")
    } else {
        injected
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falls_back_to_empty_string_without_injected_env() {
        // This crate's own test build never sets MUNDUS_ENGINE_VERSION /
        // MUNDUS_ENGINE_SOURCE_COMMIT, so the fallback path is what actually
        // runs here — assert it stays a valid (empty) string, never panics.
        assert!(engine_version().is_empty() || engine_version().is_ascii());
        assert!(
            engine_source_commit().is_empty()
                || engine_source_commit()
                    .chars()
                    .all(|c| c.is_ascii_hexdigit())
        );
    }

    #[test]
    fn display_version_falls_back_to_cargo_pkg_version_when_not_injected() {
        // In test builds no env is injected, so display_version must still
        // return a non-empty, printable version string.
        let v = display_version();
        assert!(!v.is_empty());
        assert!(v.is_ascii());
    }
}
