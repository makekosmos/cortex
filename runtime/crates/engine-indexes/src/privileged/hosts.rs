//! Managed hosts-file blocks: apply/remove a *named* set of blocked domains.
//!
//! Generic Engine feature — callers pick a block name (e.g. the focus feature
//! uses `site-block`) and declaratively set its contents. The service only
//! ever edits lines inside `# === engine:<name> BEGIN/END ===` markers and
//! never touches anything else in the file.
//!
//! Before the first modification a `hosts.engine-backup` sibling copy is
//! written (a legacy `.kepler-backup` is also honoured on reset). `reset`
//! removes every managed block.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::privileged::hosts_render as render;

#[derive(Debug)]
pub enum HostsError {
    Io(io::Error),
    InvalidInput(String),
    Verify(String),
}

impl std::fmt::Display for HostsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HostsError::Io(e) => write!(f, "IO error: {e}"),
            HostsError::InvalidInput(s) => write!(f, "invalid input: {s}"),
            HostsError::Verify(s) => write!(f, "Verify failed: {s}"),
        }
    }
}

impl std::error::Error for HostsError {}

impl From<io::Error> for HostsError {
    fn from(e: io::Error) -> Self {
        HostsError::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, HostsError>;

fn backup_paths(hosts: &Path) -> [PathBuf; 2] {
    let name = hosts.file_name().unwrap_or_default().to_os_string();
    [render::BACKUP_SUFFIX, render::LEGACY_BACKUP_SUFFIX].map(|suffix| {
        let mut candidate = name.clone();
        candidate.push(suffix);
        hosts.with_file_name(candidate)
    })
}

fn tmp_path(hosts: &Path) -> PathBuf {
    let mut name = hosts.file_name().unwrap_or_default().to_os_string();
    name.push(".tmp");
    hosts.with_file_name(name)
}

fn ensure_backup(hosts: &Path) -> Result<()> {
    let [current, legacy] = backup_paths(hosts);
    if !current.exists() && !legacy.exists() {
        fs::copy(hosts, &current)?;
    }
    Ok(())
}

fn atomic_write(hosts: &Path, content: &str) -> Result<()> {
    let tmp = tmp_path(hosts);
    fs::write(&tmp, content)?;
    // rename over an existing file works here because hosts always lives on
    // the same volume as its sibling tmp file.
    if hosts.exists() {
        let _ = fs::remove_file(hosts);
    }
    fs::rename(&tmp, hosts)?;

    let back = fs::read_to_string(hosts)?;
    if back != content {
        return Err(HostsError::Verify("readback mismatch".into()));
    }
    Ok(())
}

fn read_content(hosts: &Path) -> Result<String> {
    Ok(if hosts.exists() {
        fs::read_to_string(hosts)?
    } else {
        String::new()
    })
}

/// Domains currently in `block` inside `hosts`. Missing file → empty.
pub fn read_block(hosts: &Path, block: &str) -> Result<Vec<String>> {
    let parsed = render::parse(&read_content(hosts)?);
    Ok(parsed
        .blocks
        .iter()
        .find(|(name, _)| name == block)
        .map(|(_, domains)| domains.clone())
        .unwrap_or_default())
}

/// All managed blocks: `(name, domains)` pairs.
pub fn list_blocks(hosts: &Path) -> Result<Vec<(String, Vec<String>)>> {
    Ok(render::parse(&read_content(hosts)?).blocks)
}

/// Set `block` to exactly `domains` (validated, normalized, sorted, deduped).
/// Legacy kepler-focus sections are removed and their domains are absorbed
/// into this block — the block being applied is the successor of the old
/// focus blocklist.
/// Returns the resulting domains of the block.
pub fn apply_block(hosts: &Path, block: &str, domains: &[String]) -> Result<Vec<String>> {
    if !render::is_valid_block_name(block) {
        return Err(HostsError::InvalidInput(format!("block name: {block:?}")));
    }
    ensure_backup(hosts)?;
    let parsed = render::parse(&read_content(hosts)?);

    let mut merged: Vec<String> = Vec::new();
    for domain in domains.iter().chain(parsed.legacy_domains.iter()) {
        let normalized = render::normalize_domain(domain);
        if !render::is_blockable_domain(&normalized) {
            continue;
        }
        if !merged.iter().any(|d| d == &normalized) {
            merged.push(normalized);
        }
    }
    merged.sort();

    let mut blocks = parsed.blocks;
    match blocks.iter_mut().find(|(name, _)| name == block) {
        Some((_, existing)) => *existing = merged.clone(),
        None => blocks.push((block.to_string(), merged.clone())),
    }

    atomic_write(
        hosts,
        &render::render(&parsed.prefix, &blocks, &parsed.suffix),
    )?;
    read_block(hosts, block)
}

/// Remove `block` entirely (other blocks and all foreign lines untouched).
/// Also removes legacy kepler-focus sections. Returns the domains the block
/// had, or an empty vec.
pub fn remove_block(hosts: &Path, block: &str) -> Result<Vec<String>> {
    if !hosts.exists() {
        return Ok(Vec::new());
    }
    let parsed = render::parse(&read_content(hosts)?);
    let removed = parsed
        .blocks
        .iter()
        .find(|(name, _)| name == block)
        .map(|(_, domains)| domains.clone())
        .unwrap_or_default();
    let blocks: Vec<(String, Vec<String>)> = parsed
        .blocks
        .into_iter()
        .filter(|(name, _)| name != block)
        .collect();
    atomic_write(
        hosts,
        &render::render(&parsed.prefix, &blocks, &parsed.suffix),
    )?;
    Ok(removed)
}

/// Remove every managed block (and legacy sections), restoring the backup
/// when one exists.
pub fn reset(hosts: &Path) -> Result<()> {
    let [current_backup, legacy_backup] = backup_paths(hosts);
    let backup = if current_backup.exists() {
        Some(current_backup)
    } else if legacy_backup.exists() {
        Some(legacy_backup)
    } else {
        None
    };

    if let Some(backup) = backup {
        let content = fs::read_to_string(&backup)?;
        atomic_write(hosts, &content)?;
        // Defensive: a backup taken after we had already written markers
        // would reintroduce them — strip the managed region if so.
        if content.contains(render::MARKER_PREFIX) || content.contains(render::LEGACY_BEGIN_MARKER)
        {
            let parsed = render::parse(&content);
            atomic_write(
                hosts,
                &render::render_without_managed(&parsed.prefix, &parsed.suffix),
            )?;
        }
    } else {
        if !hosts.exists() {
            return Ok(());
        }
        let parsed = render::parse(&read_content(hosts)?);
        atomic_write(
            hosts,
            &render::render_without_managed(&parsed.prefix, &parsed.suffix),
        )?;
    }
    Ok(())
}

/// The hosts file the service edits. `MUNDUS_PRIVILEGED_HOSTS_PATH` overrides
/// it for headless tests and smoke tooling — never in production installs.
pub fn default_hosts_path() -> PathBuf {
    if let Some(p) = crate::brand::env("PRIVILEGED_HOSTS_PATH") {
        return PathBuf::from(p);
    }
    let sysroot = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into());
    PathBuf::from(sysroot).join("System32\\drivers\\etc\\hosts")
}

#[cfg(test)]
#[path = "hosts_tests.rs"]
mod tests;
