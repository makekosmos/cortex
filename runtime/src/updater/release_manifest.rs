//! Parser for the release `manifest.json` (KOS-350) — the one release
//! document `desktop/scripts/release-manifest.mjs` writes and publishes next
//! to the installers. The updater needs only `version` and the entry for its
//! own platform (`file`, `size`, `sha512`); every other field (source
//! commit, toolchain, url, target, …) is for the release tooling and is
//! ignored here, as are unknown additive keys.
//!
//! ```json
//! {
//!   "schema": "mundus-release-manifest",
//!   "schema_version": 1,
//!   "version": "0.10.6",
//!   "platforms": {
//!     "win": { "file": "Mundus-Setup-0.10.6.exe", "size": 123, "sha512": "base64…" },
//!     "mac": { "file": "Mundus-0.10.6.dmg", "size": 456, "sha512": "base64…" }
//!   }
//! }
//! ```

use std::collections::BTreeMap;

use serde::Deserialize;

use super::manifest::ManifestFile;
use super::UpdaterError;

pub(crate) const MANIFEST_SCHEMA: &str = "mundus-release-manifest";
pub(crate) const MANIFEST_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Deserialize)]
struct Document {
    schema: String,
    schema_version: u32,
    version: String,
    platforms: BTreeMap<String, PlatformEntry>,
}

#[derive(Debug, Deserialize)]
struct PlatformEntry {
    file: String,
    size: u64,
    sha512: String,
}

/// What a feed says about the newest release for one platform. `file` is
/// `None` when the release exists but has no build for this platform (for
/// example the macOS job has not attached its DMG yet).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FeedRelease {
    pub(crate) version: String,
    pub(crate) file: Option<ManifestFile>,
}

pub(crate) fn parse_release_manifest(
    text: &str,
    platform: &str,
) -> Result<FeedRelease, UpdaterError> {
    let document: Document =
        serde_json::from_str(text).map_err(|_| UpdaterError::MalformedManifest)?;
    if document.schema != MANIFEST_SCHEMA
        || document.schema_version != MANIFEST_SCHEMA_VERSION
        || document.version.trim().is_empty()
        || document.platforms.is_empty()
    {
        return Err(UpdaterError::MalformedManifest);
    }
    let file = match document.platforms.get(platform) {
        None => None,
        Some(entry) if entry.file.is_empty() || entry.sha512.is_empty() || entry.size == 0 => {
            return Err(UpdaterError::MalformedManifest);
        }
        Some(entry) => Some(ManifestFile {
            url: entry.file.clone(),
            sha512: entry.sha512.clone(),
            size: entry.size,
        }),
    };
    Ok(FeedRelease {
        version: document.version,
        file,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"{
      "schema": "mundus-release-manifest",
      "schema_version": 1,
      "product": "mundus",
      "version": "0.10.6",
      "channel": "production",
      "source": { "repository": "makekosmos/cortex", "commit": "aaaa" },
      "platforms": {
        "win": {
          "file": "Mundus-Setup-0.10.6.exe",
          "url": "https://github.com/makekosmos/cortex/releases/download/v0.10.6/Mundus-Setup-0.10.6.exe",
          "size": 122041993,
          "sha512": "CX4w1234==",
          "target": "x86_64-pc-windows-msvc"
        },
        "mac": { "file": "Mundus-0.10.6.dmg", "size": 5, "sha512": "MAC==" }
      },
      "future_field": { "ignored": true }
    }"#;

    #[test]
    fn picks_the_entry_for_the_requested_platform() {
        let win = parse_release_manifest(FIXTURE, "win").unwrap();
        assert_eq!(win.version, "0.10.6");
        let file = win.file.unwrap();
        assert_eq!(file.url, "Mundus-Setup-0.10.6.exe");
        assert_eq!(file.sha512, "CX4w1234==");
        assert_eq!(file.size, 122_041_993);
        let mac = parse_release_manifest(FIXTURE, "mac").unwrap();
        assert_eq!(mac.file.unwrap().url, "Mundus-0.10.6.dmg");
    }

    #[test]
    fn a_platform_without_a_build_has_no_file() {
        let linux = parse_release_manifest(FIXTURE, "linux").unwrap();
        assert_eq!(linux.version, "0.10.6");
        assert!(linux.file.is_none());
    }

    #[test]
    fn rejects_other_schemas_versions_and_broken_entries() {
        for text in [
            "not json",
            "{}",
            &FIXTURE.replace("\"schema_version\": 1", "\"schema_version\": 2"),
            &FIXTURE.replace("mundus-release-manifest", "other"),
            &FIXTURE.replace("\"size\": 122041993", "\"size\": 0"),
            &FIXTURE.replace("\"sha512\": \"CX4w1234==\"", "\"sha512\": \"\""),
            r#"{"schema":"mundus-release-manifest","schema_version":1,"version":"1.0.0","platforms":{}}"#,
        ] {
            assert!(
                matches!(
                    parse_release_manifest(text, "win"),
                    Err(UpdaterError::MalformedManifest)
                ),
                "{text}"
            );
        }
    }
}
