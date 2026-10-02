//! Parser/renderer unit tests (no filesystem).

use super::*;

const ORIGINAL: &str = "# Copyright (c) Microsoft Corp.\n127.0.0.1 localhost\n";
const BLOCK: &str = "site-block";
const BEGIN: &str = "# === engine:site-block BEGIN ===";
const END: &str = "# === engine:site-block END ===";

#[test]
fn parse_without_markers_returns_no_blocks() {
    let parsed = parse(ORIGINAL);
    assert!(parsed.blocks.is_empty());
    assert_eq!(parsed.suffix, "");
    assert!(parsed.legacy_domains.is_empty());
}

#[test]
fn parse_with_markers_identifies_block() {
    let content = format!(
        "{ORIGINAL}{BEGIN}\n127.0.0.1 tiktok.com\n127.0.0.1 www.tiktok.com\n{END}\n# trailing\n"
    );
    let parsed = parse(&content);
    assert!(parsed.prefix.contains("localhost"));
    assert_eq!(
        parsed.blocks,
        vec![(BLOCK.to_string(), vec!["tiktok.com".to_string()])]
    );
    assert!(parsed.suffix.contains("trailing"));
}

#[test]
fn render_drops_foreign_ip_rows_inside_markers() {
    // A managed block that already contains an injected `6.6.6.6` row must
    // not adopt it: only `127.0.0.1 <host>` rows count as managed entries.
    let content = format!(
        "{ORIGINAL}{BEGIN}\n127.0.0.1 ok.com\n6.6.6.6 login.live.com\n127.0.0.1 a \
b.example\n{END}\n"
    );
    let parsed = parse(&content);
    assert_eq!(
        parsed.blocks,
        vec![(BLOCK.to_string(), vec!["ok.com".to_string()])]
    );
    let rewritten = render(&parsed.prefix, &parsed.blocks, &parsed.suffix);
    assert!(!rewritten.contains("6.6.6.6"));
    assert!(!rewritten.contains("a b.example"));
}
