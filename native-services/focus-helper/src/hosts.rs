//! Логика работы с Windows hosts file.
//!
//! Helper модифицирует hosts только между маркерами BEGIN/END. Перед первым
//! изменением создаётся `hosts.mundus-backup` (sibling файл). На reset
//! mundus-секция удаляется (backup может остаться для recovery).
//!
//! MIGRATION(KOS-267): remove after 2026-11-01. Blocks written by the
//! MIGRATION(KOS-267): Kosmos/Kepler-era helper used `kepler-focus` markers and a
//! `.kepler-backup` file; they persist inside user hosts files, so we keep
//! recognising them and rewrite with the new markers on the next write.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const BEGIN_MARKER: &str = "# === mundus-focus BEGIN ===";
pub const END_MARKER: &str = "# === mundus-focus END ===";
pub const BACKUP_SUFFIX: &str = ".mundus-backup";

const LEGACY_BEGIN_MARKER: &str = "# === kepler-focus BEGIN ==="; // MIGRATION(KOS-267)
const LEGACY_END_MARKER: &str = "# === kepler-focus END ==="; // MIGRATION(KOS-267)
const LEGACY_BACKUP_SUFFIX: &str = ".kepler-backup"; // MIGRATION(KOS-267)

#[derive(Debug)]
pub enum HostsError {
    Io(io::Error),
    Verify(String),
}

impl std::fmt::Display for HostsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HostsError::Io(e) => write!(f, "IO error: {e}"),
            HostsError::Verify(s) => write!(f, "Verify failed: {s}"),
        }
    }
}

impl From<io::Error> for HostsError {
    fn from(e: io::Error) -> Self {
        HostsError::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, HostsError>;

fn backup_path(hosts: &Path) -> PathBuf {
    let mut name = hosts.file_name().unwrap_or_default().to_os_string();
    name.push(BACKUP_SUFFIX);
    hosts.with_file_name(name)
}

fn tmp_path(hosts: &Path) -> PathBuf {
    let mut name = hosts.file_name().unwrap_or_default().to_os_string();
    name.push(".tmp");
    hosts.with_file_name(name)
}

/// Парсит существующий contents hosts → (prefix, managed_domains, suffix).
/// Если маркеры отсутствуют — managed = пусто, prefix = весь content, suffix = пусто.
pub fn parse(content: &str) -> (String, Vec<String>, String) {
    let lines: Vec<&str> = content.lines().collect();
    let mut begin_idx: Option<usize> = None;
    let mut end_idx: Option<usize> = None;
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if begin_idx.is_none() && (trimmed == BEGIN_MARKER || trimmed == LEGACY_BEGIN_MARKER) {
            begin_idx = Some(i);
        } else if (trimmed == END_MARKER || trimmed == LEGACY_END_MARKER)
            && begin_idx.is_some()
            && end_idx.is_none()
        {
            end_idx = Some(i);
            break;
        }
    }

    let (begin, end) = match (begin_idx, end_idx) {
        (Some(b), Some(e)) => (b, e),
        _ => {
            return (content.to_string(), Vec::new(), String::new());
        }
    };

    let prefix = join_lines(&lines[..begin], content);
    let suffix = join_suffix(&lines[end + 1..]);

    let mut domains = Vec::new();
    for line in &lines[begin + 1..end] {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        // A managed row is exactly `127.0.0.1 <host>` — what
        // render_managed_block writes. Anything else inside the markers
        // (foreign `ip name` rows, extra columns) is not ours to re-render.
        let mut parts = trimmed.split_whitespace();
        if parts.next() != Some("127.0.0.1") {
            continue;
        }
        if let Some(host) = parts.next().filter(|_| parts.next().is_none()) {
            let normalized = normalize(host);
            if is_blockable_domain(&normalized)
                && !domains.iter().any(|d: &String| d == &normalized)
            {
                domains.push(normalized);
            }
        }
    }

    (prefix, domains, suffix)
}

fn join_lines(slice: &[&str], original: &str) -> String {
    if slice.is_empty() {
        return String::new();
    }
    let mut out = slice.join("\n");
    // Если оригинал заканчивался newline-перед маркером — сохраняем.
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

fn render_managed_block(domains: &[String]) -> String {
    let mut out = String::new();
    out.push_str(BEGIN_MARKER);
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
    out.push_str(END_MARKER);
    out
}

fn normalize(domain: &str) -> String {
    let d = domain.trim().to_lowercase();
    d.strip_prefix("www.").unwrap_or(&d).to_string()
}

/// A blockable entry is a plain DNS hostname: dot-separated ASCII labels,
/// alphanumeric with `-`, no whitespace/control/IP literals. The service pipe
/// is open to every authenticated user, so request strings are attacker input —
/// anything else (e.g. a newline) would smuggle a full `ip name` row into the
/// hosts file, which resolves names to arbitrary addresses as SYSTEM.
fn is_blockable_domain(name: &str) -> bool {
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

/// Возвращает новый content с заданным managed списком (sorted, deduped).
pub fn render(prefix: &str, domains: &[String], suffix: &str) -> String {
    let mut deduped: Vec<String> = Vec::new();
    for d in domains {
        let n = normalize(d);
        if !is_blockable_domain(&n) {
            continue;
        }
        if !deduped.iter().any(|x| x == &n) {
            deduped.push(n);
        }
    }
    deduped.sort();

    let mut out = String::new();
    out.push_str(prefix);
    if !prefix.is_empty() && !prefix.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&render_managed_block(&deduped));
    if !suffix.is_empty() {
        if !suffix.starts_with('\n') {
            out.push('\n');
        }
        out.push_str(suffix);
    } else {
        out.push('\n');
    }
    out
}

/// Возвращает новый content с пустой managed-секцией удалённой.
pub fn render_without_managed(prefix: &str, suffix: &str) -> String {
    let mut out = String::from(prefix);
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    // Удаляем ведущий newline у suffix чтобы не образовалась двойная пустая строка.
    let suf = suffix.trim_start_matches('\n');
    out.push_str(suf);
    out
}

fn ensure_backup(hosts: &Path) -> Result<()> {
    let bk = backup_path(hosts);
    if !bk.exists() {
        // MIGRATION(KOS-267): adopt a backup left by the legacy helper.
        let legacy_bk = hosts.with_file_name({
            let mut name = hosts.file_name().unwrap_or_default().to_os_string();
            name.push(LEGACY_BACKUP_SUFFIX);
            name
        });
        if legacy_bk.exists() {
            fs::rename(&legacy_bk, &bk)?;
            return Ok(());
        }
        fs::copy(hosts, &bk)?;
    }
    Ok(())
}

fn atomic_write(hosts: &Path, content: &str) -> Result<()> {
    let tmp = tmp_path(hosts);
    fs::write(&tmp, content)?;
    // На Windows rename поверх existing работает только для одного volume —
    // hosts всегда на C:, tmp в той же папке, поэтому ok.
    if hosts.exists() {
        let _ = fs::remove_file(hosts);
    }
    fs::rename(&tmp, hosts)?;

    // Verify
    let back = fs::read_to_string(hosts)?;
    if back != content {
        return Err(HostsError::Verify("readback mismatch".into()));
    }
    Ok(())
}

pub fn read_active_domains(hosts: &Path) -> Result<Vec<String>> {
    let content = if hosts.exists() {
        fs::read_to_string(hosts)?
    } else {
        String::new()
    };
    let (_, domains, _) = parse(&content);
    Ok(domains)
}

pub fn add_domains(hosts: &Path, new_domains: &[String]) -> Result<Vec<String>> {
    ensure_backup(hosts)?;
    let content = fs::read_to_string(hosts)?;
    let (prefix, mut domains, suffix) = parse(&content);
    for d in new_domains {
        let n = normalize(d);
        if !is_blockable_domain(&n) {
            continue;
        }
        if !domains.iter().any(|x| x == &n) {
            domains.push(n);
        }
    }
    let new_content = render(&prefix, &domains, &suffix);
    atomic_write(hosts, &new_content)?;
    read_active_domains(hosts)
}

pub fn remove_domains(hosts: &Path, to_remove: &[String]) -> Result<Vec<String>> {
    let content = if hosts.exists() {
        fs::read_to_string(hosts)?
    } else {
        return Ok(Vec::new());
    };
    let (prefix, mut domains, suffix) = parse(&content);
    let remove_set: Vec<String> = to_remove.iter().map(|d| normalize(d)).collect();
    domains.retain(|d| !remove_set.iter().any(|r| r == d));

    let new_content = if domains.is_empty() {
        render_without_managed(&prefix, &suffix)
    } else {
        render(&prefix, &domains, &suffix)
    };
    atomic_write(hosts, &new_content)?;
    read_active_domains(hosts)
}

pub fn reset(hosts: &Path) -> Result<Vec<String>> {
    // MIGRATION(KOS-267): accept a legacy `.kepler-backup` too.
    let bk = {
        let primary = backup_path(hosts);
        if primary.exists() {
            primary
        } else {
            let mut name = hosts.file_name().unwrap_or_default().to_os_string();
            name.push(LEGACY_BACKUP_SUFFIX);
            hosts.with_file_name(name)
        }
    };
    if bk.exists() {
        let content = fs::read_to_string(&bk)?;
        atomic_write(hosts, &content)?;
        // Дополнительно убедимся, что markers отсутствуют — если backup
        // был сделан после того, как focus уже модифицировал hosts (не
        // должно случаться, но defensive).
        let (prefix, _, suffix) = parse(&content);
        if content.contains(BEGIN_MARKER) || content.contains(LEGACY_BEGIN_MARKER) {
            let cleaned = render_without_managed(&prefix, &suffix);
            atomic_write(hosts, &cleaned)?;
        }
    } else {
        // Backup отсутствует → просто удаляем managed секцию.
        let content = if hosts.exists() {
            fs::read_to_string(hosts)?
        } else {
            return Ok(Vec::new());
        };
        let (prefix, _, suffix) = parse(&content);
        let cleaned = render_without_managed(&prefix, &suffix);
        atomic_write(hosts, &cleaned)?;
    }
    read_active_domains(hosts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup(initial: &str) -> (TempDir, PathBuf) {
        let dir = TempDir::new().unwrap();
        let hosts = dir.path().join("hosts");
        fs::write(&hosts, initial).unwrap();
        (dir, hosts)
    }

    const ORIGINAL: &str = "# Copyright (c) Microsoft Corp.\n127.0.0.1 localhost\n";

    #[test]
    fn parse_without_markers_returns_empty_managed() {
        let (_, domains, suffix) = parse(ORIGINAL);
        assert!(domains.is_empty());
        assert_eq!(suffix, "");
    }

    #[test]
    fn parse_with_markers_identifies_section() {
        let content = format!(
            "{ORIGINAL}{BEGIN_MARKER}\n127.0.0.1 tiktok.com\n127.0.0.1 www.tiktok.com\n{END_MARKER}\n# trailing\n"
        );
        let (prefix, domains, suffix) = parse(&content);
        assert!(prefix.contains("localhost"));
        assert_eq!(domains, vec!["tiktok.com".to_string()]);
        assert!(suffix.contains("trailing"));
    }

    #[test]
    fn add_inserts_block_between_markers() {
        let (_d, hosts) = setup(ORIGINAL);
        let active = add_domains(&hosts, &["tiktok.com".into(), "twitter.com".into()]).unwrap();
        assert_eq!(active, vec!["tiktok.com".to_string(), "twitter.com".into()]);

        let content = fs::read_to_string(&hosts).unwrap();
        assert!(content.contains(BEGIN_MARKER));
        assert!(content.contains(END_MARKER));
        assert!(content.contains("127.0.0.1 tiktok.com"));
        assert!(content.contains("127.0.0.1 www.tiktok.com"));
        assert!(content.contains("127.0.0.1 twitter.com"));
        assert!(content.contains("127.0.0.1 www.twitter.com"));
        assert!(content.contains("127.0.0.1 localhost"));
    }

    #[test]
    fn add_creates_backup_once() {
        let (_d, hosts) = setup(ORIGINAL);
        add_domains(&hosts, &["a.com".into()]).unwrap();
        let bk = backup_path(&hosts);
        assert!(bk.exists());
        let bk_content_1 = fs::read_to_string(&bk).unwrap();
        assert_eq!(bk_content_1, ORIGINAL);

        add_domains(&hosts, &["b.com".into()]).unwrap();
        let bk_content_2 = fs::read_to_string(&bk).unwrap();
        assert_eq!(bk_content_2, ORIGINAL, "backup must not be overwritten");
    }

    #[test]
    fn remove_keeps_other_managed_entries() {
        let (_d, hosts) = setup(ORIGINAL);
        add_domains(&hosts, &["a.com".into(), "b.com".into()]).unwrap();
        let active = remove_domains(&hosts, &["a.com".into()]).unwrap();
        assert_eq!(active, vec!["b.com".to_string()]);

        let content = fs::read_to_string(&hosts).unwrap();
        assert!(!content.contains("127.0.0.1 a.com"));
        assert!(content.contains("127.0.0.1 b.com"));
        assert!(content.contains("127.0.0.1 localhost"));
    }

    #[test]
    fn remove_last_collapses_section() {
        let (_d, hosts) = setup(ORIGINAL);
        add_domains(&hosts, &["a.com".into()]).unwrap();
        remove_domains(&hosts, &["a.com".into()]).unwrap();
        let content = fs::read_to_string(&hosts).unwrap();
        assert!(!content.contains(BEGIN_MARKER));
        assert!(!content.contains(END_MARKER));
        assert!(content.contains("127.0.0.1 localhost"));
    }

    #[test]
    fn reset_restores_backup_and_drops_section() {
        let (_d, hosts) = setup(ORIGINAL);
        add_domains(&hosts, &["a.com".into(), "b.com".into()]).unwrap();
        let active = reset(&hosts).unwrap();
        assert!(active.is_empty());
        let content = fs::read_to_string(&hosts).unwrap();
        assert_eq!(content, ORIGINAL);
    }

    #[test]
    fn reset_without_backup_still_clears_section() {
        let (_d, hosts) = setup(&format!(
            "{ORIGINAL}{BEGIN_MARKER}\n127.0.0.1 x.com\n{END_MARKER}\n"
        ));
        // Backup отсутствует.
        let active = reset(&hosts).unwrap();
        assert!(active.is_empty());
        let content = fs::read_to_string(&hosts).unwrap();
        assert!(!content.contains(BEGIN_MARKER));
        assert!(content.contains("127.0.0.1 localhost"));
    }

    #[test]
    fn legacy_markers_are_recognised_and_rewritten() {
        // MIGRATION(KOS-267): a hosts file edited by the Kosmos-era helper
        // still parses; the next write uses the new markers and the old
        // backup file is adopted under the new name.
        let (_d, hosts) = setup(&format!(
            "{ORIGINAL}{LEGACY_BEGIN_MARKER}\n127.0.0.1 x.com\n{LEGACY_END_MARKER}\n"
        ));
        let legacy_bk = hosts.with_file_name("hosts.kepler-backup"); // MIGRATION(KOS-267)
        fs::write(&legacy_bk, ORIGINAL).unwrap();

        let active = add_domains(&hosts, &["y.com".into()]).unwrap();
        assert_eq!(active, vec!["x.com".to_string(), "y.com".to_string()]);
        let content = fs::read_to_string(&hosts).unwrap();
        assert!(content.contains(BEGIN_MARKER));
        assert!(!content.contains(LEGACY_BEGIN_MARKER));
        assert!(!legacy_bk.exists());
        assert_eq!(
            fs::read_to_string(hosts.with_file_name("hosts.mundus-backup")).unwrap(),
            ORIGINAL
        );
    }

    #[test]
    fn add_is_idempotent() {
        let (_d, hosts) = setup(ORIGINAL);
        let a1 = add_domains(&hosts, &["tiktok.com".into()]).unwrap();
        let content_1 = fs::read_to_string(&hosts).unwrap();
        let a2 = add_domains(&hosts, &["tiktok.com".into()]).unwrap();
        let content_2 = fs::read_to_string(&hosts).unwrap();
        assert_eq!(a1, a2);
        assert_eq!(content_1, content_2);
    }

    #[test]
    fn add_normalizes_www_prefix() {
        let (_d, hosts) = setup(ORIGINAL);
        let active = add_domains(&hosts, &["www.tiktok.com".into(), "tiktok.com".into()]).unwrap();
        assert_eq!(active, vec!["tiktok.com".to_string()]);
    }

    #[test]
    fn read_active_on_clean_hosts_returns_empty() {
        let (_d, hosts) = setup(ORIGINAL);
        let active = read_active_domains(&hosts).unwrap();
        assert!(active.is_empty());
    }

    #[test]
    fn add_rejects_entries_that_would_inject_hosts_rows() {
        let (_d, hosts) = setup(ORIGINAL);
        // A request string containing a newline would otherwise land a full
        // `<ip> <name>` row — redirecting any domain to an attacker address.
        let active = add_domains(
            &hosts,
            &[
                "ok.com".into(),
                "x\n6.6.6.6 login.live.com".into(),
                "10.0.0.7 plain-ip.example".into(),
                "two words.example".into(),
                "#comment.example".into(),
            ],
        )
        .unwrap();
        assert_eq!(active, vec!["ok.com".to_string()]);
        let content = fs::read_to_string(&hosts).unwrap();
        assert!(content.contains("127.0.0.1 ok.com"));
        assert!(!content.contains("6.6.6.6"));
        assert!(!content.contains("login.live.com"));
        assert!(!content.contains("plain-ip.example"));
        assert!(!content.contains("words.example"));
        assert!(!content.contains("#comment.example"));
    }

    #[test]
    fn add_rejects_ip_literals_and_malformed_names() {
        let (_d, hosts) = setup(ORIGINAL);
        let active = add_domains(
            &hosts,
            &[
                "192.168.0.1".into(),
                "::1".into(),
                "-bad.example".into(),
                "bad-.example".into(),
                "under_score.example".into(),
                "..example".into(),
                "a..example".into(),
            ],
        )
        .unwrap();
        assert!(active.is_empty());
        let content = fs::read_to_string(&hosts).unwrap();
        for bad in [
            "192.168.0.1",
            "::1",
            "-bad.example",
            "bad-.example",
            "under_score.example",
            "..example",
            "a..example",
        ] {
            assert!(!content.contains(bad), "unexpected hosts entry: {bad}");
        }
    }

    #[test]
    fn render_drops_foreign_ip_rows_inside_markers() {
        // A managed block that already contains an injected `6.6.6.6` row must
        // not adopt it: only `127.0.0.1 <host>` rows count as managed entries.
        let content = format!(
            "{ORIGINAL}{BEGIN_MARKER}\n127.0.0.1 ok.com\n6.6.6.6 login.live.com\n127.0.0.1 a b.example\n{END_MARKER}\n"
        );
        let (prefix, domains, suffix) = parse(&content);
        assert_eq!(domains, vec!["ok.com".to_string()]);
        let rewritten = render(&prefix, &domains, &suffix);
        assert!(!rewritten.contains("6.6.6.6"));
        assert!(!rewritten.contains("a b.example"));
    }
}
