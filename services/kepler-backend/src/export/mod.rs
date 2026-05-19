// Phase 7 — universal per-type data export module.
//
// Plug-in архитектура:
//   - trait `Converter` — описывает один конвертер (object_type → file format).
//   - `registry()` — статический список всех конвертеров (lazy-init через OnceLock).
//   - `convert_with(converter_id, format, objects, dest_dir)` — dispatch по id.
//
// Все конвертеры read-only: они получают &[ArkObject] на вход, пишут файлы в
// dest_dir, ничего не пишут обратно в ARK.

use ark_core::types::ArkObject;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub mod game_json;
pub mod note_md;
pub mod tag_json;
pub mod task_csv;
pub mod task_md;
pub mod time_entry_csv;

/// Результат одного `convert` вызова — что записали, сколько байт, какие
/// ошибки встретили (но не упали — partial success возможен).
#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertResult {
    pub files_written: Vec<PathBuf>,
    pub bytes: u64,
    pub errors: Vec<String>,
}

impl ConvertResult {
    pub fn push_file(&mut self, path: PathBuf, bytes: u64) {
        self.files_written.push(path);
        self.bytes += bytes;
    }

    pub fn push_error(&mut self, err: impl Into<String>) {
        self.errors.push(err.into());
    }
}

/// Один конвертер: object_type → file format.
pub trait Converter: Send + Sync {
    /// Уникальный id, e.g. "note_md". Используется в WS `export.run`.
    fn id(&self) -> &'static str;
    /// Человекочитаемое имя для UI, e.g. "Заметки → Markdown".
    fn display_name(&self) -> &'static str;
    /// Тип объектов ARK, с которыми работает конвертер.
    fn object_type(&self) -> &'static str;
    /// Формат по умолчанию (расширение без точки).
    fn default_format(&self) -> &'static str;
    /// Все поддерживаемые форматы.
    fn supported_formats(&self) -> &'static [&'static str];
    /// Выполнить конвертацию. dest_dir гарантированно существует (создаётся
    /// диспатчером). Конвертер сам решает, сколько файлов писать.
    fn convert(&self, objects: &[ArkObject], format: &str, dest_dir: &Path) -> ConvertResult;
}

/// Метадата converter'а для `export.list` ответа.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConverterInfo {
    pub converter_id: String,
    pub object_type: String,
    pub display_name: String,
    pub default_format: String,
    pub supported_formats: Vec<String>,
}

fn build_registry() -> Vec<Box<dyn Converter>> {
    vec![
        Box::new(note_md::NoteMdConverter),
        Box::new(task_md::TaskMdConverter),
        Box::new(task_csv::TaskCsvConverter),
        Box::new(time_entry_csv::TimeEntryCsvConverter),
        Box::new(tag_json::TagJsonConverter),
        Box::new(game_json::GameJsonConverter),
    ]
}

fn registry() -> &'static Vec<Box<dyn Converter>> {
    static REGISTRY: OnceLock<Vec<Box<dyn Converter>>> = OnceLock::new();
    REGISTRY.get_or_init(build_registry)
}

pub fn list_converters() -> Vec<ConverterInfo> {
    registry()
        .iter()
        .map(|c| ConverterInfo {
            converter_id: c.id().to_string(),
            object_type: c.object_type().to_string(),
            display_name: c.display_name().to_string(),
            default_format: c.default_format().to_string(),
            supported_formats: c.supported_formats().iter().map(|s| s.to_string()).collect(),
        })
        .collect()
}

pub fn find_converter(converter_id: &str) -> Option<&'static dyn Converter> {
    registry()
        .iter()
        .find(|c| c.id() == converter_id)
        .map(|b| b.as_ref())
}

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
        // Truncate at char boundary to avoid splitting multi-byte characters.
        let mut boundary = 120;
        while !out.is_char_boundary(boundary) {
            boundary -= 1;
        }
        out.truncate(boundary);
    }
    out
}

/// Sanitize CSV cell value to prevent formula injection in spreadsheet apps.
///
/// Spreadsheet applications (Excel, Google Sheets) interpret cells that start
/// with `=`, `+`, `-`, or `@` as formulas, which can execute arbitrary code.
/// Prefix such values with a TAB character to force literal interpretation.
pub(crate) fn csv_safe_cell(value: &str) -> std::borrow::Cow<'_, str> {
    if value.starts_with(['=', '+', '-', '@']) {
        std::borrow::Cow::Owned(format!("\t{value}"))
    } else {
        std::borrow::Cow::Borrowed(value)
    }
}

/// Подобрать уникальное имя файла в dest_dir с заданным stem + extension.
/// Если файл существует — добавляет суффикс `-2`, `-3`, ...
/// Если все суффиксы до 9999 заняты — использует timestamp суффикс.
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
    // All numeric suffixes exhausted — use epoch-millis as guaranteed-unique fallback.
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
    fn registry_lists_all_six_converters() {
        let list = list_converters();
        let ids: Vec<&str> = list.iter().map(|c| c.converter_id.as_str()).collect();
        assert!(ids.contains(&"note_md"), "note_md missing");
        assert!(ids.contains(&"task_md"), "task_md missing");
        assert!(ids.contains(&"task_csv"), "task_csv missing");
        assert!(ids.contains(&"time_entry_csv"), "time_entry_csv missing");
        assert!(ids.contains(&"tag_json"), "tag_json missing");
        assert!(ids.contains(&"game_json"), "game_json missing");
        assert_eq!(list.len(), 6);
    }

    #[test]
    fn find_converter_resolves_by_id() {
        assert!(find_converter("note_md").is_some());
        assert!(find_converter("does_not_exist").is_none());
    }

    #[test]
    fn sanitize_filename_strips_banned_chars() {
        assert_eq!(sanitize_filename("foo/bar:baz?.md"), "foobarbaz.md");
        assert_eq!(sanitize_filename(""), "untitled");
        assert_eq!(sanitize_filename("...."), "untitled");
        assert_eq!(sanitize_filename("Привет мир"), "Привет мир");
    }

    #[test]
    fn sanitize_filename_truncates_at_char_boundary() {
        // Each Cyrillic character is 2 bytes. 61 chars = 122 bytes > 120.
        // truncate(120) would split the 61st char without boundary check → panic.
        let input = "А".repeat(61);
        let out = sanitize_filename(&input);
        assert!(out.len() <= 120, "len={}", out.len());
        assert!(std::str::from_utf8(out.as_bytes()).is_ok(), "not valid UTF-8");
        // 120 bytes / 2 bytes-per-char = 60 chars
        assert_eq!(out, "А".repeat(60));
    }

    #[test]
    fn csv_safe_cell_sanitizes_formula_prefix() {
        assert_eq!(csv_safe_cell("=SUM(1+1)"), "\t=SUM(1+1)");
        assert_eq!(csv_safe_cell("+bad"), "\t+bad");
        assert_eq!(csv_safe_cell("-also-bad"), "\t-also-bad");
        assert_eq!(csv_safe_cell("@user"), "\t@user");
        // Safe values unchanged.
        assert_eq!(csv_safe_cell("Hello"), "Hello");
        assert_eq!(csv_safe_cell("Buy milk"), "Buy milk");
        assert_eq!(csv_safe_cell(""), "");
        assert_eq!(csv_safe_cell("100"), "100");
    }

    #[test]
    fn test_unique_path_exhaustion_fallback() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path();
        // Create stem.ext and stem-2.ext … stem-9999.ext.
        std::fs::write(dir.join("title.md"), b"").unwrap();
        for n in 2..10_000u32 {
            std::fs::write(dir.join(format!("title-{n}.md")), b"").unwrap();
        }
        let path = unique_path(dir, "title", "md");
        // Must not return a path that already exists.
        assert!(!path.exists(), "unique_path returned an existing path: {:?}", path);
    }
}
