use std::borrow::Cow;
use std::path::{Path, PathBuf};

/// Sanitize строку для использования в имени файла (Windows-safe).
/// Замечание: не трогает пробелы — markdown OK с пробелами на Win.
pub(crate) fn sanitize_filename(input: &str) -> String {
    let banned = ['/', '\\', ':', '*', '?', '"', '<', '>', '|'];
    let mut out: String = input
        .chars()
        .filter(|c| !banned.contains(c) && !c.is_control())
        .collect();
    out = out.trim().trim_matches('.').to_string();
    if out.is_empty() {
        out = "untitled".to_string();
    }
    if out.len() > 120 {
        let mut boundary = 120;
        while !out.is_char_boundary(boundary) {
            boundary -= 1;
        }
        out.truncate(boundary);
    }
    out
}

/// Sanitize CSV cell value to prevent formula injection in spreadsheet apps.
pub(crate) fn csv_safe_cell(value: &str) -> Cow<'_, str> {
    if value.starts_with(['=', '+', '-', '@']) {
        Cow::Owned(format!("\t{value}"))
    } else {
        Cow::Borrowed(value)
    }
}

/// Подобрать уникальное имя файла в dest_dir с заданным stem + extension.
pub(crate) fn unique_path(dest_dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let candidate = dest_dir.join(format!("{stem}.{ext}"));
    if !candidate.exists() {
        return candidate;
    }
    for n in 2..10_000 {
        let c = dest_dir.join(format!("{stem}-{n}.{ext}"));
        if !c.exists() {
            return c;
        }
    }
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    dest_dir.join(format!("{stem}-{ts}.{ext}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_filename_strips_banned_chars() {
        assert_eq!(sanitize_filename("foo/bar:baz?.md"), "foobarbaz.md");
        assert_eq!(sanitize_filename(""), "untitled");
        assert_eq!(sanitize_filename("...."), "untitled");
        assert_eq!(sanitize_filename("Привет мир"), "Привет мир");
    }

    #[test]
    fn sanitize_filename_truncates_at_char_boundary() {
        let input = "А".repeat(61);
        let out = sanitize_filename(&input);
        assert!(out.len() <= 120, "len={}", out.len());
        assert!(std::str::from_utf8(out.as_bytes()).is_ok());
        assert_eq!(out, "А".repeat(60));
    }

    #[test]
    fn csv_safe_cell_sanitizes_formula_prefix() {
        assert_eq!(csv_safe_cell("=SUM(1+1)"), "\t=SUM(1+1)");
        assert_eq!(csv_safe_cell("+bad"), "\t+bad");
        assert_eq!(csv_safe_cell("-also-bad"), "\t-also-bad");
        assert_eq!(csv_safe_cell("@user"), "\t@user");
        assert_eq!(csv_safe_cell("Hello"), "Hello");
        assert_eq!(csv_safe_cell("Buy milk"), "Buy milk");
        assert_eq!(csv_safe_cell(""), "");
        assert_eq!(csv_safe_cell("100"), "100");
    }

    #[test]
    fn unique_path_uses_next_numeric_suffix() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dir = tmp.path();
        std::fs::write(dir.join("title.md"), b"").unwrap();
        std::fs::write(dir.join("title-2.md"), b"").unwrap();

        assert_eq!(unique_path(dir, "title", "md"), dir.join("title-3.md"));
    }

    #[test]
    fn unique_path_exhaustion_fallback() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dir = tmp.path();
        std::fs::write(dir.join("title.md"), b"").unwrap();
        for n in 2..10_000u32 {
            std::fs::write(dir.join(format!("title-{n}.md")), b"").unwrap();
        }
        let path = unique_path(dir, "title", "md");
        assert!(
            !path.exists(),
            "unique_path returned existing path: {path:?}"
        );
    }
}
