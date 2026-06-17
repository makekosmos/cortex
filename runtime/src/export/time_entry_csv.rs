// time_entry_obj → CSV. Один файл time-entries.csv.
// Колонки: id, title, started_at, ended_at, duration_minutes, task_id, source

use super::{csv_safe_cell, ConvertResult, Converter};
use ark_core::types::ArkObject;
use std::path::Path;

pub struct TimeEntryCsvConverter;

impl Converter for TimeEntryCsvConverter {
    fn id(&self) -> &'static str {
        "time_entry_csv"
    }
    fn display_name(&self) -> &'static str {
        "Тайм-трекинг → CSV"
    }
    fn object_type(&self) -> &'static str {
        "time_entry_obj"
    }
    fn default_format(&self) -> &'static str {
        "csv"
    }
    fn supported_formats(&self) -> &'static [&'static str] {
        &["csv"]
    }

    fn convert(&self, objects: &[ArkObject], _format: &str, dest_dir: &Path) -> ConvertResult {
        let mut result = ConvertResult::default();
        let path = dest_dir.join("time-entries.csv");
        let mut wtr = match csv::WriterBuilder::new().from_path(&path) {
            Ok(w) => w,
            Err(e) => {
                result.push_error(format!("open {path:?}: {e}"));
                return result;
            }
        };
        let header = [
            "id",
            "title",
            "started_at",
            "ended_at",
            "duration_minutes",
            "task_id",
            "source",
        ];
        if let Err(e) = wtr.write_record(header) {
            result.push_error(format!("write header: {e}"));
            return result;
        }
        for obj in objects {
            if obj.deleted_at.is_some() {
                continue;
            }
            let p = &obj.props_json;
            let started = p.get("started_at").and_then(|v| v.as_str()).unwrap_or("");
            let ended = p.get("ended_at").and_then(|v| v.as_str()).unwrap_or("");
            let duration = p
                .get("duration_minutes")
                .or_else(|| p.get("duration_min"))
                .map(|v| v.to_string())
                .unwrap_or_default();
            let task_id = p.get("task_id").and_then(|v| v.as_str()).unwrap_or("");
            let source = p.get("source").and_then(|v| v.as_str()).unwrap_or("");
            if let Err(e) = wtr.write_record([
                obj.id.as_str(),
                csv_safe_cell(&obj.title).as_ref(),
                started,
                ended,
                duration.as_str(),
                task_id,
                csv_safe_cell(source).as_ref(),
            ]) {
                result.push_error(format!("write row {}: {e}", obj.id));
            }
        }
        if let Err(e) = wtr.flush() {
            result.push_error(format!("flush: {e}"));
        }
        drop(wtr);
        if let Ok(meta) = std::fs::metadata(&path) {
            result.push_file(path, meta.len());
        } else {
            result.push_file(path, 0);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    fn sample(id: &str, title: &str, props: serde_json::Value) -> ArkObject {
        ArkObject {
            id: id.to_string(),
            type_id: "time_entry_obj".to_string(),
            title: title.to_string(),
            content_json: json!({}),
            props_json: props,
            created_at: "2026-05-18T10:00:00Z".to_string(),
            updated_at: "2026-05-18T10:30:00Z".to_string(),
            deleted_at: None,
        }
    }

    #[test]
    fn writes_entries() {
        let dir = tempdir().unwrap();
        let e1 = sample(
            "e1",
            "Coding",
            json!({
                "started_at": "2026-05-18T09:00:00Z",
                "ended_at": "2026-05-18T10:30:00Z",
                "duration_minutes": 90,
                "task_id": "t1",
                "source": "pomodoro"
            }),
        );
        let e2 = sample(
            "e2",
            "Reading",
            json!({
                "started_at": "2026-05-18T11:00:00Z",
                "duration_minutes": 25,
                "source": "manual"
            }),
        );
        let res = TimeEntryCsvConverter.convert(&[e1, e2], "csv", dir.path());
        assert!(res.errors.is_empty(), "errors: {:?}", res.errors);
        assert_eq!(res.files_written.len(), 1);
        let content = std::fs::read_to_string(&res.files_written[0]).unwrap();
        assert!(content.starts_with("id,title,started_at,ended_at,duration_minutes,task_id,source"));
        assert!(
            content.contains("e1,Coding,2026-05-18T09:00:00Z,2026-05-18T10:30:00Z,90,t1,pomodoro")
        );
        assert!(content.contains("e2,Reading,2026-05-18T11:00:00Z,,25,,manual"));
    }

    #[test]
    fn handles_empty_input() {
        let dir = tempdir().unwrap();
        let res = TimeEntryCsvConverter.convert(&[], "csv", dir.path());
        assert!(res.errors.is_empty());
        assert_eq!(res.files_written.len(), 1);
        let content = std::fs::read_to_string(&res.files_written[0]).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 1, "only header expected");
    }
}
