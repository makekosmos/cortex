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
mod util;

pub(crate) use util::{csv_safe_cell, sanitize_filename, unique_path};

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
            supported_formats: c
                .supported_formats()
                .iter()
                .map(|s| s.to_string())
                .collect(),
        })
        .collect()
}

pub fn find_converter(converter_id: &str) -> Option<&'static dyn Converter> {
    registry()
        .iter()
        .find(|c| c.id() == converter_id)
        .map(|b| b.as_ref())
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
    fn converter_metadata_matches_registry_entries() {
        for info in list_converters() {
            let converter = find_converter(&info.converter_id).expect("listed converter resolves");
            assert_eq!(info.converter_id, converter.id());
            assert_eq!(info.object_type, converter.object_type());
            assert_eq!(info.display_name, converter.display_name());
            assert_eq!(info.default_format, converter.default_format());
            assert_eq!(
                info.supported_formats,
                converter
                    .supported_formats()
                    .iter()
                    .map(|format| format.to_string())
                    .collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn convert_result_accumulates_files_bytes_and_errors() {
        let mut result = ConvertResult::default();
        result.push_file(PathBuf::from("first.md"), 10);
        result.push_file(PathBuf::from("second.md"), 7);
        result.push_error("bad row");

        assert_eq!(
            result.files_written,
            vec![PathBuf::from("first.md"), PathBuf::from("second.md")]
        );
        assert_eq!(result.bytes, 17);
        assert_eq!(result.errors, vec!["bad row"]);
    }
}
