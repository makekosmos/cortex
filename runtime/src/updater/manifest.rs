//! Parser for electron-builder's `latest.yml` channel file. Same flat format
//! `desktop/scripts/verify-release-channel.mjs` hand-parses on the release
//! side (see `parseLatestYmlByHand` there) — ported to Rust rather than
//! pulling in a YAML crate, since the format is fixed and small:
//!
//! ```yaml
//! version: 0.5.3
//! files:
//!   - url: Mundus-Setup-0.5.3.exe
//!     sha512: CX4w...==
//!     size: 122041993
//! path: Mundus-Setup-0.5.3.exe
//! sha512: CX4w...==
//! releaseDate: '2026-06-18T12:18:15.656Z'
//! ```

use super::UpdaterError;

/// One `files[]` entry: installer filename (relative — joined with the
/// release's asset base URL by the caller), base64 sha512 and byte size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ManifestFile {
    pub(crate) url: String,
    pub(crate) sha512: String,
    pub(crate) size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LatestManifest {
    pub(crate) version: String,
    pub(crate) files: Vec<ManifestFile>,
}

impl LatestManifest {
    /// The Windows build publishes exactly one NSIS installer under
    /// `files[0]` — the top-level legacy `path`/`sha512` fields duplicate it
    /// for older clients and are intentionally not read here.
    pub(crate) fn primary_file(&self) -> Result<&ManifestFile, UpdaterError> {
        self.files.first().ok_or(UpdaterError::MalformedManifest)
    }
}

fn strip_quotes(value: &str) -> &str {
    value
        .strip_prefix(['\'', '"'])
        .and_then(|v| v.strip_suffix(['\'', '"']))
        .unwrap_or(value)
}

fn key_value(trimmed: &str) -> Option<(&str, &str)> {
    let (key, rest) = trimmed.split_once(':')?;
    Some((key.trim(), strip_quotes(rest.trim())))
}

/// Mirrors `parseLatestYmlByHand` in `verify-release-channel.mjs`: top-level
/// `key: value` pairs, a `files:` key that opens a list of `- key: value`
/// blocks indented under it. No other YAML feature (anchors, flow style,
/// nested maps beyond one level) is needed or handled.
pub(crate) fn parse_latest_yml(text: &str) -> Result<LatestManifest, UpdaterError> {
    let mut version: Option<String> = None;
    let mut files = Vec::new();
    let mut current: Option<(Option<String>, Option<String>, Option<u64>)> = None;
    let mut in_files = false;

    let flush = |current: &mut Option<(Option<String>, Option<String>, Option<u64>)>,
                 files: &mut Vec<ManifestFile>| {
        if let Some((Some(url), Some(sha512), Some(size))) = current.take() {
            files.push(ManifestFile { url, sha512, size });
        }
    };

    for raw_line in text.lines() {
        let line = raw_line.split('#').next().unwrap_or("");
        if line.trim().is_empty() {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        let trimmed = line.trim();

        if indent == 0 {
            flush(&mut current, &mut files);
            let Some((key, value)) = key_value(trimmed) else {
                in_files = false;
                continue;
            };
            match key {
                "version" => {
                    version = Some(value.to_owned());
                    in_files = false;
                }
                "files" => in_files = true,
                _ => in_files = false,
            }
            continue;
        }

        if !in_files {
            continue;
        }

        let rest = trimmed
            .strip_prefix('-')
            .map(str::trim_start)
            .unwrap_or(trimmed);
        let is_new_item = trimmed.starts_with('-');
        let Some((key, value)) = key_value(rest) else {
            continue;
        };
        if is_new_item {
            flush(&mut current, &mut files);
            current = Some((None, None, None));
        }
        let Some(entry) = current.as_mut() else {
            continue;
        };
        match key {
            "url" => entry.0 = Some(value.to_owned()),
            "sha512" => entry.1 = Some(value.to_owned()),
            "size" => entry.2 = value.parse().ok(),
            _ => {}
        }
    }
    flush(&mut current, &mut files);

    let version = version.ok_or(UpdaterError::MalformedManifest)?;
    if files.is_empty() {
        return Err(UpdaterError::MalformedManifest);
    }
    Ok(LatestManifest { version, files })
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"version: 0.5.3
files:
  - url: Mundus-Setup-0.5.3.exe
    sha512: CX4w1234==
    size: 122041993
path: Mundus-Setup-0.5.3.exe
sha512: CX4w1234==
releaseDate: '2026-06-18T12:18:15.656Z'
"#;

    #[test]
    fn parses_the_electron_builder_flat_format() {
        let manifest = parse_latest_yml(FIXTURE).unwrap();
        assert_eq!(manifest.version, "0.5.3");
        assert_eq!(manifest.files.len(), 1);
        let file = manifest.primary_file().unwrap();
        assert_eq!(file.url, "Mundus-Setup-0.5.3.exe");
        assert_eq!(file.sha512, "CX4w1234==");
        assert_eq!(file.size, 122_041_993);
    }

    #[test]
    fn handles_multiple_file_entries_and_keeps_the_first() {
        let text = "version: 1.2.0\nfiles:\n  - url: Mundus-Setup-1.2.0.exe\n    sha512: AAA\n    size: 10\n  - url: extra.zip\n    sha512: BBB\n    size: 20\n";
        let manifest = parse_latest_yml(text).unwrap();
        assert_eq!(manifest.files.len(), 2);
        assert_eq!(
            manifest.primary_file().unwrap().url,
            "Mundus-Setup-1.2.0.exe"
        );
    }

    #[test]
    fn rejects_missing_version_or_empty_files() {
        assert!(matches!(
            parse_latest_yml("files:\n  - url: a\n    sha512: b\n    size: 1\n"),
            Err(UpdaterError::MalformedManifest)
        ));
        assert!(matches!(
            parse_latest_yml("version: 1.0.0\n"),
            Err(UpdaterError::MalformedManifest)
        ));
    }

    #[test]
    fn ignores_comments_and_blank_lines() {
        let text = "# channel file\nversion: 2.0.0\n\nfiles:\n  # single asset\n  - url: a.exe\n    sha512: c\n    size: 5\n";
        let manifest = parse_latest_yml(text).unwrap();
        assert_eq!(manifest.version, "2.0.0");
        assert_eq!(manifest.primary_file().unwrap().size, 5);
    }
}
