//! Pure render/parse logic for managed hosts-file blocks.
//!
//! The Engine owns one contiguous "managed region" in the hosts file,
//! containing one or more named blocks:
//!
//! ```text
//! # === engine:<name> BEGIN ===
//! 127.0.0.1 domain.example
//! 127.0.0.1 www.domain.example
//! # === engine:<name> END ===
//! ```
//!
//! Lines outside our markers are never touched. Rows inside a block that are
//! not exactly `127.0.0.1 <host>` are foreign (hand-edits or injections) and
//! are dropped on the next render — never adopted as ours.

/// Marker prefix for Engine-owned blocks: `# === engine:<name> BEGIN ===`.
/// Brand-neutral on purpose — the hosts file survives product renames.
pub const MARKER_PREFIX: &str = "# === engine:";
pub const BEGIN_SUFFIX: &str = " BEGIN ===";
pub const END_SUFFIX: &str = " END ===";
pub const BACKUP_SUFFIX: &str = ".engine-backup";

/// Markers written by the removed kepler-focus-helper. Their contents are
/// absorbed into the block being applied, then the legacy section is dropped.
// MIGRATION(KOS-267): remove after 2026-11-01.
pub const LEGACY_BEGIN_MARKER: &str = "# === kepler-focus BEGIN ===";
pub const LEGACY_END_MARKER: &str = "# === kepler-focus END ===";
pub const LEGACY_BACKUP_SUFFIX: &str = ".kepler-backup";

/// A parsed hosts file: text before the managed region, the named blocks in
/// file order, text after the region, and whether legacy markers were seen.
#[derive(Debug, Default)]
pub struct ParsedHosts {
    pub prefix: String,
    /// `(name, domains)` pairs in file order. Duplicate names merge.
    pub blocks: Vec<(String, Vec<String>)>,
    pub suffix: String,
    /// Domains found inside legacy (pre-Engine) managed markers.
    pub legacy_domains: Vec<String>,
}

fn marker_kind(line: &str) -> Option<(Option<String>, bool)> {
    let trimmed = line.trim();
    if trimmed == LEGACY_BEGIN_MARKER {
        return Some((None, true));
    }
    if trimmed == LEGACY_END_MARKER {
        return Some((None, false));
    }
    if let Some(rest) = trimmed.strip_prefix(MARKER_PREFIX) {
        if let Some(name) = rest.strip_suffix(BEGIN_SUFFIX) {
            return Some((Some(name.to_string()), true));
        }
        if let Some(name) = rest.strip_suffix(END_SUFFIX) {
            return Some((Some(name.to_string()), false));
        }
    }
    None
}

/// Valid block names are short DNS-safe slugs: `site-block`, `adult`, ...
/// The name is rendered inside marker lines, so whitespace or `=` would let a
/// crafted name corrupt the marker structure.
pub fn is_valid_block_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 32
        && bytes
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
}

/// Parse `content` into (prefix, blocks, suffix, legacy_domains).
///
/// The managed region spans from the first BEGIN marker to the last END
/// marker; everything between is treated as managed and re-rendered (foreign
/// rows inside are dropped, matching the hardened single-block behavior).
/// A file with no markers parses as entirely prefix.
pub fn parse(content: &str) -> ParsedHosts {
    let lines: Vec<&str> = content.lines().collect();
    let mut first_begin: Option<usize> = None;
    let mut last_end: Option<usize> = None;
    let mut blocks: Vec<(String, Vec<String>)> = Vec::new();
    let mut legacy_domains = Vec::new();
    let mut current: Option<Option<String>> = None; // None name = legacy block

    for (i, line) in lines.iter().enumerate() {
        match marker_kind(line) {
            Some((name, true)) if current.is_none() => {
                if first_begin.is_none() {
                    first_begin = Some(i);
                }
                current = Some(name);
            }
            Some((_, false)) if current.is_some() => {
                current = None;
                last_end = Some(i);
            }
            Some((name, true)) => {
                // Nested/aborted block: the previous one stays unclosed and
                // the whole parse falls back to "all prefix" below.
                current = Some(name);
            }
            _ => {
                if let Some(open) = &current {
                    let trimmed = line.trim();
                    if trimmed.is_empty() || trimmed.starts_with('#') {
                        continue;
                    }
                    // A managed row is exactly `127.0.0.1 <host>`.
                    let mut parts = trimmed.split_whitespace();
                    if parts.next() != Some("127.0.0.1") {
                        continue;
                    }
                    if let Some(host) = parts.next().filter(|_| parts.next().is_none()) {
                        let normalized = normalize(host);
                        if !is_blockable_domain(&normalized) {
                            continue;
                        }
                        match open {
                            Some(name) => {
                                let idx = match blocks.iter().position(|(n, _)| n == name) {
                                    Some(i) => i,
                                    None => {
                                        blocks.push((name.clone(), Vec::new()));
                                        blocks.len() - 1
                                    }
                                };
                                let list = &mut blocks[idx].1;
                                if !list.iter().any(|d| d == &normalized) {
                                    list.push(normalized);
                                }
                            }
                            None => {
                                if !legacy_domains.iter().any(|d| d == &normalized) {
                                    legacy_domains.push(normalized);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let (begin, end) = match (first_begin, last_end) {
        (Some(b), Some(e)) if e >= b => (b, e),
        _ => {
            // No markers at all, or an unclosed BEGIN (truncated/hand-edited
            // file): keep the entire content as prefix so nothing is lost.
            return ParsedHosts {
                prefix: content.to_string(),
                ..ParsedHosts::default()
            };
        }
    };

    ParsedHosts {
        prefix: join_lines(&lines[..begin], content),
        blocks,
        suffix: join_suffix(&lines[(end + 1).min(lines.len())..]),
        legacy_domains,
    }
}

fn join_lines(slice: &[&str], original: &str) -> String {
    if slice.is_empty() {
        return String::new();
    }
    let mut out = slice.join("\n");
    if !out.is_empty() && !original.is_empty() {
        out.push('\n');
    }
    out
}

fn join_suffix(slice: &[&str]) -> String {
    if slice.is_empty() {
        return String::new();
    }
    let mut out = String::from("\n");
    out.push_str(&slice.join("\n"));
    out
}

fn render_block(name: &str, domains: &[String]) -> String {
    let mut out = String::new();
    out.push_str(MARKER_PREFIX);
    out.push_str(name);
    out.push_str(BEGIN_SUFFIX);
    out.push('\n');
    for d in domains {
        out.push_str("127.0.0.1 ");
        out.push_str(d);
        out.push('\n');
        if !d.starts_with("www.") {
            out.push_str("127.0.0.1 www.");
            out.push_str(d);
            out.push('\n');
        }
    }
    out.push_str(MARKER_PREFIX);
    out.push_str(name);
    out.push_str(END_SUFFIX);
    out
}

fn normalize(domain: &str) -> String {
    let d = domain.trim().to_lowercase();
    d.strip_prefix("www.").unwrap_or(&d).to_string()
}

pub fn normalize_domain(domain: &str) -> String {
    normalize(domain)
}

/// A blockable entry is a plain DNS hostname: dot-separated ASCII labels,
/// alphanumeric with `-`, no whitespace/control/IP literals. The service pipe
/// is reachable by exactly one local user, but request strings are still
/// attacker input — anything else (e.g. a newline) would smuggle a full
/// `ip name` row into the hosts file, resolving names to arbitrary
/// addresses as SYSTEM.
pub fn is_blockable_domain(name: &str) -> bool {
    if name.is_empty() || name.len() > 253 || name.parse::<std::net::IpAddr>().is_ok() {
        return false;
    }
    name.split('.').all(|label| {
        let bytes = label.as_bytes();
        !bytes.is_empty()
            && bytes.len() <= 63
            && bytes[0].is_ascii_alphanumeric()
            && bytes[bytes.len() - 1].is_ascii_alphanumeric()
            && bytes
                .iter()
                .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'-')
    })
}

/// Render the full file: prefix + every non-empty block (sorted by name,
/// blank-line separated) + suffix. Empty-domain blocks are omitted (i.e.
/// removed); with no blocks at all the managed region disappears entirely.
pub fn render(prefix: &str, blocks: &[(String, Vec<String>)], suffix: &str) -> String {
    // A block whose name fails validation was not written by us (our writer
    // only emits valid names) — drop it rather than preserve foreign
    // pseudo-managed content forever.
    let mut sorted: Vec<&(String, Vec<String>)> = blocks
        .iter()
        .filter(|(name, domains)| is_valid_block_name(name) && !domains.is_empty())
        .collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));

    if sorted.is_empty() {
        return render_without_managed(prefix, suffix);
    }

    let mut out = String::new();
    out.push_str(prefix);
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    for (i, (name, domains)) in sorted.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(&render_block(name, domains));
        out.push('\n');
    }
    if !suffix.is_empty() {
        out.push_str(suffix.trim_start_matches('\n'));
    }
    out
}

/// Render a file with no managed region at all.
pub fn render_without_managed(prefix: &str, suffix: &str) -> String {
    let mut out = String::from(prefix);
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(suffix.trim_start_matches('\n'));
    out
}

#[cfg(test)]
#[path = "hosts_render_tests.rs"]
mod tests;
