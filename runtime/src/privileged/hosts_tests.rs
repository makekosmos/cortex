use super::*;
use tempfile::TempDir;

fn setup(initial: &str) -> (TempDir, PathBuf) {
    let dir = TempDir::new().unwrap();
    let hosts = dir.path().join("hosts");
    fs::write(&hosts, initial).unwrap();
    (dir, hosts)
}

const ORIGINAL: &str = "# Copyright (c) Microsoft Corp.\n127.0.0.1 localhost\n";
const BLOCK: &str = "site-block";
const BEGIN: &str = "# === engine:site-block BEGIN ===";
const END: &str = "# === engine:site-block END ===";

#[test]
fn apply_inserts_block_between_markers() {
    let (_d, hosts) = setup(ORIGINAL);
    let active = apply_block(&hosts, BLOCK, &["tiktok.com".into(), "twitter.com".into()]).unwrap();
    assert_eq!(active, vec!["tiktok.com".to_string(), "twitter.com".into()]);

    let content = fs::read_to_string(&hosts).unwrap();
    assert!(content.contains(BEGIN));
    assert!(content.contains(END));
    assert!(content.contains("127.0.0.1 tiktok.com"));
    assert!(content.contains("127.0.0.1 www.tiktok.com"));
    assert!(content.contains("127.0.0.1 twitter.com"));
    assert!(content.contains("127.0.0.1 localhost"));
}

#[test]
fn apply_creates_backup_once() {
    let (_d, hosts) = setup(ORIGINAL);
    apply_block(&hosts, BLOCK, &["a.com".into()]).unwrap();
    let backup = backup_paths(&hosts)[0].clone();
    assert!(backup.exists());
    assert_eq!(fs::read_to_string(&backup).unwrap(), ORIGINAL);

    apply_block(&hosts, BLOCK, &["b.com".into()]).unwrap();
    assert_eq!(
        fs::read_to_string(&backup).unwrap(),
        ORIGINAL,
        "backup must not be overwritten"
    );
}

#[test]
fn second_named_block_coexists() {
    let (_d, hosts) = setup(ORIGINAL);
    apply_block(&hosts, "site-block", &["a.com".into()]).unwrap();
    apply_block(&hosts, "other-block", &["b.com".into()]).unwrap();
    let content = fs::read_to_string(&hosts).unwrap();
    assert!(content.contains("# === engine:site-block BEGIN ==="));
    assert!(content.contains("# === engine:other-block BEGIN ==="));
    assert!(content.contains("127.0.0.1 a.com"));
    assert!(content.contains("127.0.0.1 b.com"));
    assert_eq!(read_block(&hosts, "site-block").unwrap(), vec!["a.com"]);
    assert_eq!(read_block(&hosts, "other-block").unwrap(), vec!["b.com"]);
}

#[test]
fn remove_one_block_keeps_others() {
    let (_d, hosts) = setup(ORIGINAL);
    apply_block(&hosts, "site-block", &["a.com".into()]).unwrap();
    apply_block(&hosts, "other-block", &["b.com".into()]).unwrap();
    remove_block(&hosts, "site-block").unwrap();
    let content = fs::read_to_string(&hosts).unwrap();
    assert!(!content.contains("site-block"));
    assert!(content.contains("127.0.0.1 b.com"));
    assert!(content.contains("127.0.0.1 localhost"));
}

#[test]
fn remove_last_block_collapses_region() {
    let (_d, hosts) = setup(ORIGINAL);
    apply_block(&hosts, BLOCK, &["a.com".into()]).unwrap();
    remove_block(&hosts, BLOCK).unwrap();
    let content = fs::read_to_string(&hosts).unwrap();
    assert!(!content.contains("engine:"));
    assert!(content.contains("127.0.0.1 localhost"));
}

#[test]
fn apply_is_declarative_not_additive() {
    let (_d, hosts) = setup(ORIGINAL);
    apply_block(&hosts, BLOCK, &["a.com".into(), "b.com".into()]).unwrap();
    let active = apply_block(&hosts, BLOCK, &["b.com".into()]).unwrap();
    assert_eq!(active, vec!["b.com".to_string()]);
    let content = fs::read_to_string(&hosts).unwrap();
    assert!(!content.contains("127.0.0.1 a.com"));
}

#[test]
fn reset_restores_backup_and_drops_region() {
    let (_d, hosts) = setup(ORIGINAL);
    apply_block(&hosts, BLOCK, &["a.com".into(), "b.com".into()]).unwrap();
    reset(&hosts).unwrap();
    assert_eq!(fs::read_to_string(&hosts).unwrap(), ORIGINAL);
}

#[test]
fn reset_without_backup_still_clears_region() {
    let (_d, hosts) = setup(&format!("{ORIGINAL}{BEGIN}\n127.0.0.1 x.com\n{END}\n"));
    reset(&hosts).unwrap();
    let content = fs::read_to_string(&hosts).unwrap();
    assert!(!content.contains("engine:"));
    assert!(content.contains("127.0.0.1 localhost"));
}

#[test]
fn apply_is_idempotent() {
    let (_d, hosts) = setup(ORIGINAL);
    let a1 = apply_block(&hosts, BLOCK, &["tiktok.com".into()]).unwrap();
    let c1 = fs::read_to_string(&hosts).unwrap();
    let a2 = apply_block(&hosts, BLOCK, &["tiktok.com".into()]).unwrap();
    let c2 = fs::read_to_string(&hosts).unwrap();
    assert_eq!(a1, a2);
    assert_eq!(c1, c2);
}

#[test]
fn apply_normalizes_www_prefix() {
    let (_d, hosts) = setup(ORIGINAL);
    let active = apply_block(
        &hosts,
        BLOCK,
        &["www.tiktok.com".into(), "tiktok.com".into()],
    )
    .unwrap();
    assert_eq!(active, vec!["tiktok.com".to_string()]);
}

#[test]
fn apply_rejects_invalid_block_names() {
    let (_d, hosts) = setup(ORIGINAL);
    for bad in [
        "",
        "UPPER",
        "has space",
        "a=b",
        "semi;colon",
        "x\n# === engine:y BEGIN ===",
    ] {
        assert!(
            apply_block(&hosts, bad, &["a.com".into()]).is_err(),
            "accepted invalid block name {bad:?}"
        );
    }
    assert!(fs::read_to_string(&hosts).unwrap() == ORIGINAL);
}

#[test]
fn apply_rejects_entries_that_would_inject_hosts_rows() {
    let (_d, hosts) = setup(ORIGINAL);
    // A request string containing a newline would otherwise land a full
    // `<ip> <name>` row — redirecting any domain to an attacker address.
    let active = apply_block(
        &hosts,
        BLOCK,
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
fn apply_rejects_ip_literals_and_malformed_names() {
    let (_d, hosts) = setup(ORIGINAL);
    let active = apply_block(
        &hosts,
        BLOCK,
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
fn legacy_kepler_section_is_absorbed_then_removed() {
    // MIGRATION(KOS-267): legacy `kepler-focus` sections keep working until
    // the next apply, which absorbs their domains and drops the old markers.
    let (_d, hosts) = setup(&format!(
        "{ORIGINAL}# === kepler-focus BEGIN ===\n127.0.0.1 legacy.com\n# === kepler-focus END ===\n"
    ));
    let active = apply_block(&hosts, BLOCK, &["new.com".into()]).unwrap();
    assert_eq!(
        active,
        vec!["legacy.com".to_string(), "new.com".to_string()]
    );
    let content = fs::read_to_string(&hosts).unwrap();
    assert!(!content.contains("kepler-focus"));
    assert!(content.contains("127.0.0.1 legacy.com"));
}

#[test]
fn remove_also_drops_legacy_section() {
    let (_d, hosts) = setup(&format!(
        "{ORIGINAL}# === kepler-focus BEGIN ===\n127.0.0.1 legacy.com\n# === kepler-focus END ===\n"
    ));
    remove_block(&hosts, BLOCK).unwrap();
    let content = fs::read_to_string(&hosts).unwrap();
    assert!(!content.contains("kepler-focus"));
    assert!(!content.contains("legacy.com"));
    assert!(content.contains("127.0.0.1 localhost"));
}

#[test]
fn unclosed_marker_region_is_preserved_verbatim() {
    // A truncated file (BEGIN without END) must not lose the trailing lines.
    let content = format!("{ORIGINAL}{BEGIN}\n127.0.0.1 x.com\n# user footer\n");
    let (_d, hosts) = setup(&content);
    apply_block(&hosts, "other-block", &["a.com".into()]).unwrap();
    let written = fs::read_to_string(&hosts).unwrap();
    assert!(written.contains("# user footer"));
    assert!(written.contains("# === engine:other-block BEGIN ==="));
}

#[test]
fn legacy_backup_is_honoured_on_reset() {
    let (_d, hosts) = setup(ORIGINAL);
    // Simulate a pre-Engine backup: `.kepler-backup` exists, our new-suffix
    // backup does not, and the file already carries a legacy section.
    fs::write(
        hosts.with_file_name("hosts.kepler-backup"),
        "127.0.0.1 localhost\n",
    )
    .unwrap();
    reset(&hosts).unwrap();
    assert_eq!(fs::read_to_string(&hosts).unwrap(), "127.0.0.1 localhost\n");
}
