// SHA256SUMS.txt parsing: sha256sum line format, host-target line selection,
// asset-name version extraction, and the redirect-Location → tag check.

/// One well-formed `sha256sum` line: `<hex>  <file>` or `<hex> *<file>`.
pub struct SumsEntry {
    pub sha256: String,
    pub name: String,
}

/// The sums entry that matches a target, with the version parsed out of
/// the asset name (`<stem>-<version>-<target>.<ext>`).
pub struct AssetMatch {
    pub version: String,
    pub asset: String,
    pub sha256: String,
}

/// Parse `sha256sum`-format lines; malformed lines are skipped.
pub fn parse_sums(body: &str) -> Vec<SumsEntry> {
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
            Some(SumsEntry {
                sha256: hash.to_ascii_lowercase(),
                name: name.to_owned(),
            })
        })
        .collect()
}

/// Find the sums line for `target`.
pub fn select_asset(
    sums: &[SumsEntry],
    desc: &NativeAppDescriptor,
    target: &str,
) -> Option<AssetMatch> {
    let prefix = format!("{}-", desc.asset_stem);
    let suffix = format!(
        "-{}.{}",
        target,
        NativeAppDescriptor::asset_extension(target)
    );
    sums.iter().find_map(|entry| {
        let version = entry.name.strip_prefix(&prefix)?.strip_suffix(&suffix)?;
        semver::Version::parse(version).ok()?;
        Some(AssetMatch {
            version: version.to_owned(),
            asset: entry.name.clone(),
            sha256: entry.sha256.clone(),
        })
    })
}

/// Cross-check the redirect tag against the asset-name version, then bundle
/// everything an install needs. The tag is always authoritative: it comes
/// from the release endpoint's own redirect.
pub(crate) fn info_from_sums(
    desc: &NativeAppDescriptor,
    tag: &str,
    body: &str,
    target: &str,
) -> Result<ReleaseInfo, ReleaseError> {
    let found = select_asset(&parse_sums(body), desc, target)
        .ok_or(ReleaseError::Invalid("no asset for target"))?;
    if desc.version_from_tag(tag).as_deref() != Some(found.version.as_str()) {
        return Err(ReleaseError::Invalid("tag/version mismatch"));
    }
    Ok(ReleaseInfo {
        tag: tag.to_owned(),
        version: found.version,
        asset: found.asset,
        sha256: found.sha256,
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
    // Same host AND scheme — a downgrade to http on the release host is as
    // bad as a redirect elsewhere.
    if url.host_str() != base.host_str() || url.scheme() != base.scheme() {
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
