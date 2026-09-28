//! Ports of `markdown-vault.test.ts` and `markdown-file-operations.test.ts`.

use super::*;
use crate::handle_relative_fs::open_root;
use std::fs;
use std::path::PathBuf;

fn fixture() -> (tempfile::TempDir, crate::handle_relative_fs::RootHandle) {
    let td = tempfile::tempdir().unwrap();
    let root = open_root(td.path()).unwrap();
    (td, root)
}

#[test]
fn default_name_falls_back_and_sanitizes() {
    assert_eq!(safe_markdown_default_name(None), "eden-object.md");
    assert_eq!(safe_markdown_default_name(Some("   ")), "eden-object.md");
    assert_eq!(
        safe_markdown_default_name(Some("../bad:name")),
        "bad-name.md"
    );
    assert_eq!(safe_markdown_default_name(Some("note.MD")), "note.MD");
}

#[test]
fn relative_path_accepts_nested_and_rejects_escapes() {
    assert_eq!(
        parse_relative_path("notes/today.md").unwrap(),
        vec!["notes", "today.md"]
    );
    // Backslashes normalize to separators (TS `replace(/\\/g, "/")` parity).
    assert_eq!(parse_relative_path("a\\b.md").unwrap(), vec!["a", "b.md"]);
    for bad in [
        "../outside.md",
        "..",
        "C:/outside.md",
        "/outside.md",
        "a/../b.md",
        "a\\..\\b.md",
        "con/.hidden/../x.md",
        "",
    ] {
        assert!(parse_relative_path(bad).is_err(), "{bad}");
    }
}

#[test]
fn source_path_resolution_matches_ts_rules() {
    let absolute = std::env::temp_dir().join("image.png");
    assert_eq!(
        resolve_vault_source_path(absolute.to_str().unwrap()),
        Some(absolute.clone())
    );
    assert_eq!(resolve_vault_source_path(""), None);
    assert_eq!(resolve_vault_source_path("   "), None);
    assert_eq!(resolve_vault_source_path("image.png"), None);
    assert_eq!(resolve_vault_source_path("relative/dir/x.png"), None);
    assert_eq!(resolve_vault_source_path("https://x/i.png"), None);
    let url = local_image_url(&absolute.to_string_lossy());
    assert_eq!(resolve_vault_source_path(&url), Some(absolute.clone()));
    // fileURLToPath shape: `file:///C:/…` on Windows, `file:///tmp/…` on Unix.
    let slashed = absolute.to_string_lossy().replace('\\', "/");
    let file_url = format!(
        "file://{}{slashed}",
        if slashed.starts_with('/') { "" } else { "/" }
    );
    assert_eq!(resolve_vault_source_path(&file_url), Some(absolute));
    // Non-image local-image URLs are rejected (extension allow-list).
    let txt_url = format!("{LOCAL_IMAGE_PROTOCOL}://file/%2Fetc%2Fpasswd.txt");
    assert_eq!(resolve_vault_source_path(&txt_url), None);
}

#[test]
fn source_path_with_multibyte_prefix_is_rejected_without_panic() {
    // Bytes 0..5 of "文件:…"/"файл:…" split a multi-byte char — the `file:`
    // scheme check used to slice `trimmed[..5]` and panic on such input
    // (reachable via `filesystem.vault.export` files[].sourcePath).
    assert_eq!(resolve_vault_source_path("文件:foo.png"), None);
    assert_eq!(resolve_vault_source_path("файл:x.png"), None);
    assert_eq!(resolve_vault_source_path("文件:server/x.png"), None);
}

#[test]
fn scan_loads_markdown_and_skips_ignored_dirs() {
    let (td, root) = fixture();
    fs::create_dir_all(td.path().join(".git")).unwrap();
    fs::create_dir_all(td.path().join("notes")).unwrap();
    fs::create_dir_all(td.path().join("node_modules/x")).unwrap();
    fs::write(td.path().join("notes/a.md"), "# A").unwrap();
    fs::write(td.path().join(".git/ignored.md"), "# ignored").unwrap();
    fs::write(td.path().join("node_modules/x/b.md"), "# nm").unwrap();
    let scan = scan_root(&root, td.path()).unwrap();
    assert_eq!(scan.files.len(), 1);
    assert_eq!(scan.files[0].relative_path, "notes/a.md");
    assert_eq!(scan.files[0].content, "# A");
}

#[test]
fn scan_decodes_non_utf8_markdown_lossily() {
    // TS parity: `readFileSync(path, "utf8")` replaces malformed bytes with
    // U+FFFD instead of throwing — one legacy-encoded note must not abort
    // the whole vault scan.
    let (td, root) = fixture();
    fs::write(td.path().join("good.md"), "# ok").unwrap();
    // Windows-1251 "Привет" — invalid UTF-8 byte sequence.
    fs::write(
        td.path().join("bad.md"),
        [0xCFu8, 0xF0, 0xE8, 0xE2, 0xE5, 0xF2],
    )
    .unwrap();
    let scan = scan_root(&root, td.path()).unwrap();
    assert_eq!(scan.files.len(), 2);
    let bad = scan.files.iter().find(|f| f.name == "bad.md").unwrap();
    assert!(bad.content.contains('\u{FFFD}'));
}

#[test]
fn scan_skips_oversized_and_collects_image_metadata() {
    let (td, root) = fixture();
    fs::write(
        td.path().join("huge.png"),
        vec![0u8; MARKDOWN_IMAGE_MAX_BYTES as usize + 1],
    )
    .unwrap();
    // Minimal PNG header: signature + IHDR length+tag + 640x480.
    let mut png = b"\x89PNG\x0d\x0a\x1a\x0a\x00\x00\x00\x0dIHDR".to_vec();
    png.extend_from_slice(&640u32.to_be_bytes());
    png.extend_from_slice(&480u32.to_be_bytes());
    fs::write(td.path().join("pic.png"), &png).unwrap();
    let scan = scan_root(&root, td.path()).unwrap();
    assert_eq!(scan.images.len(), 1);
    let image = &scan.images[0];
    assert_eq!(image.relative_path, "pic.png");
    assert_eq!(image.mime_type, "image/png");
    // Engine-servable URL, not a raw path: decodes back to the absolute file.
    assert!(image
        .file_url
        .starts_with(&format!("{LOCAL_IMAGE_PROTOCOL}://file/")));
    let decoded = resolve_vault_source_path(&image.file_url).unwrap();
    assert_eq!(decoded, td.path().join("pic.png"));
    assert_eq!((image.width, image.height), (Some(640), Some(480)));
}

#[test]
fn export_writes_text_and_copies_sources() {
    let (out_td, out_root) = fixture();
    let (src_td, _src_root) = fixture();
    fs::write(src_td.path().join("approved.png"), b"image").unwrap();
    let source = src_td.path().join("approved.png");
    let files = vec![
        VaultExportFile::Text {
            components: vec!["notes".into(), "a.md".into()],
            content: "# A".into(),
        },
        VaultExportFile::Copy {
            components: vec!["assets".into(), "a.png".into()],
            source,
        },
    ];
    assert_eq!(export_files(&out_root, &files).unwrap(), 2);
    assert_eq!(fs::read(out_td.path().join("notes/a.md")).unwrap(), b"# A");
    assert_eq!(
        fs::read(out_td.path().join("assets/a.png")).unwrap(),
        b"image"
    );
}

#[test]
fn export_rejects_dupes_and_skips_missing_sources() {
    let (_td, root) = fixture();
    let files = vec![
        VaultExportFile::Text {
            components: vec!["a.md".into()],
            content: "x".into(),
        },
        VaultExportFile::Text {
            components: vec!["a.md".into()],
            content: "y".into(),
        },
    ];
    assert!(export_files(&root, &files).is_err());
    let files = vec![VaultExportFile::Copy {
        components: vec!["gone.png".into()],
        source: PathBuf::from("/nonexistent/definitely-missing.png"),
    }];
    assert_eq!(export_files(&root, &files).unwrap(), 0);
}

#[cfg(unix)]
#[test]
fn export_refuses_to_write_through_symlinked_dirs() {
    let (td, root) = fixture();
    let outside = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(outside.path(), td.path().join("linked")).unwrap();
    let files = vec![VaultExportFile::Text {
        components: vec!["linked".into(), "escape.md".into()],
        content: "x".into(),
    }];
    assert!(export_files(&root, &files).is_err());
    assert!(!outside.path().join("escape.md").exists());
}

#[test]
fn image_dimensions_cover_supported_formats() {
    // GIF89a 4x2
    let mut gif = b"GIF89a".to_vec();
    gif.extend_from_slice(&4u16.to_le_bytes());
    gif.extend_from_slice(&2u16.to_le_bytes());
    assert_eq!(super::image_dims::image_dimensions(&gif), Some((4, 2)));
    // JPEG SOF0: FF D8 FF C0 <len=17> <prec> <h> <w>
    let jpg = [
        0xff, 0xd8, 0xff, 0xc0, 0x00, 0x11, 0x08, 0x01, 0xe0, 0x02, 0x80, 0x07,
    ];
    assert_eq!(super::image_dims::image_dimensions(&jpg), Some((640, 480)));
    // WEBP VP8X: w=LE24(24)+1, h=LE24(27)+1
    let mut webp = b"RIFF\x00\x00\x00\x00WEBPVP8X".to_vec();
    webp.resize(30, 0);
    webp[24] = 3;
    webp[27] = 1;
    assert_eq!(super::image_dims::image_dimensions(&webp), Some((4, 2)));
    assert_eq!(super::image_dims::image_dimensions(b"not an image"), None);
}
