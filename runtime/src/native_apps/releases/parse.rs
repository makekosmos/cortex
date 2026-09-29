// SHA256SUMS.txt parsing: sha256sum line format, host-target line selection,
// asset-name version extraction, and the redirect-Location → tag check.

/// Parse `sha256sum`-format lines (`<hex>  <file>` or `<hex> *<file>`) into
/// (lowercased hash, filename) pairs; malformed lines are skipped.
pub fn parse_sums(body: &str) -> Vec<(String, String)> {
    body.lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let hash = parts.next()?;
            if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
                return None;
            }
            let name = parts.next()?.trim_start_matches('*');
            if name.is_empty() {
                return None;
            }
            Some((hash.to_ascii_lowercase(), name.to_owned()))
        })
        .collect()
}

/// Find the sums line for `target` and read the version out of the asset
/// name (`<stem>-<version>-<target>.<ext>`).
pub fn select_asset(
    sums: &[(String, String)],
    desc: &NativeAppDescriptor,
    target: &str,
) -> Option<(String, String, String)> {
    let prefix = format!("{}-", desc.asset_stem);
    let suffix = format!(
        "-{}.{}",
        target,
        NativeAppDescriptor::asset_extension(target)
    );
    sums.iter().find_map(|(hash, name)| {
        let version = name.strip_prefix(&prefix)?.strip_suffix(&suffix)?;
        semver::Version::parse(version).ok()?;
        Some((version.to_owned(), name.clone(), hash.clone()))
    })
}

/// Cross-check the redirect tag against the asset-name version, then bundle
/// everything an install needs.
pub(crate) fn info_from_sums(
    desc: &NativeAppDescriptor,
    tag: Option<&str>,
    body: &str,
    target: &str,
) -> Result<ReleaseInfo, ReleaseError> {
    let (version, asset, sha256) = select_asset(&parse_sums(body), desc, target)
        .ok_or(ReleaseError::Invalid("no asset for target"))?;
    if let Some(tag) = tag {
        if desc.version_from_tag(tag).as_deref() != Some(version.as_str()) {
            return Err(ReleaseError::Invalid("tag/version mismatch"));
        }
    }
    Ok(ReleaseInfo {
        tag: tag
            .map(str::to_owned)
            .unwrap_or_else(|| desc.release_tag(&version)),
        version,
        asset,
        sha256,
    })
}

/// Extract `<tag>` from a `releases/download/<tag>/SHA256SUMS.txt` Location.
/// The redirect target must stay on the release host — a redirect elsewhere
/// would let a middlebox pick the tag.
pub(crate) fn tag_from_location(
    base: &reqwest::Url,
    repository: &str,
    location: &str,
) -> Option<String> {
    let url = base.join(location).ok()?;
    if url.host_str() != base.host_str() {
        return None;
    }
    let prefix = format!("/{repository}/releases/download/");
    let tag = url
        .path()
        .strip_prefix(&prefix)?
        .strip_suffix("/SHA256SUMS.txt")?;
    if tag.is_empty()
        || tag.len() > 64
        || !tag
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
    {
        return None;
    }
    Some(tag.to_owned())
}
