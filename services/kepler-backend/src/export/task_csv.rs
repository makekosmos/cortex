// task_obj → CSV. Один файл tasks.csv.
// Колонки: id, title, project_id, area_id, scheduled_date, deadline, completed, priority, tags

use super::{csv_safe_cell, ConvertResult, Converter};
use ark_core::types::ArkObject;
use std::path::Path;

pub struct TaskCsvConverter;

impl Converter for TaskCsvConverter {
    fn id(&self) -> &'static str {
        "task_csv"
    }
    fn display_name(&self) -> &'static str {
        "Задачи → CSV"
    }
    fn object_type(&self) -> &'static str {
        "task_obj"
    }
    fn default_format(&self) -> &'static str {
        "csv"
    }
    fn supported_formats(&self) -> &'static [&'static str] {
        &["csv"]
    }

    fn convert(&self, objects: &[ArkObject], _format: &str, dest_dir: &Path) -> ConvertResult {
        let mut result = ConvertResult::default();
        let path = dest_dir.join("tasks.csv");
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
            "project_id",
            "area_id",
            "scheduled_date",
            "deadline",
            "completed",
            "priority",
            "tags",
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
            let project_id = p.get("project_id").and_then(|v| v.as_str()).unwrap_or("");
            let area_id = p.get("area_id").and_then(|v| v.as_str()).unwrap_or("");
            let scheduled = p.get("scheduled_date").and_then(|v| v.as_str()).unwrap_or("");
            let deadline = p.get("deadline").and_then(|v| v.as_str()).unwrap_or("");
            let completed = p
                .get("completed")
                .and_then(|v| v.as_bool())
                .or_else(|| p.get("is_completed").and_then(|v| v.as_bool()))
                .unwrap_or(false);
            let priority = p
                .get("priority")
                .map(|v| v.to_string())
                .unwrap_or_default();
            let tags = p
                .get("tags")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect::<Vec<_>>()
                        .join("|")
                })
                .unwrap_or_default();
            if let Err(e) = wtr.write_record([
                obj.id.as_str(),
                csv_safe_cell(&obj.title).as_ref(),
                project_id,
                area_id,
                scheduled,
                deadline,
                if completed { "true" } else { "false" },
                priority.as_str(),
                csv_safe_cell(&tags).as_ref(),
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
            type_id: "task_obj".to_string(),
            title: title.to_string(),
            content_json: json!({}),
            props_json: props,
            created_at: "2026-05-18T10:00:00Z".to_string(),
            updated_at: "2026-05-18T10:30:00Z".to_string(),
            deleted_at: None,
        }
    }

    #[test]
    fn writes_csv_header_and_two_rows() {
        let dir = tempdir().unwrap();
        let t1 = sample(
            "t1",
            "Buy milk",
            json!({
                "project_id": "p1",
                "scheduled_date": "2026-05-20",
                "priority": 2,
                "completed": false,
                "tags": ["home", "errands"]
            }),
        );
        let t2 = sample(
            "t2",
            "Done thing",
            json!({ "completed": true, "priority": 0 }),
        );
        let res = TaskCsvConverter.convert(&[t1, t2], "csv", dir.path());
        assert!(res.errors.is_empty(), "errors: {:?}", res.errors);
        assert_eq!(res.files_written.len(), 1);
        let content = std::fs::read_to_string(&res.files_written[0]).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert!(lines[0].starts_with("id,title,project_id"));
        assert!(content.contains("t1,Buy milk,p1,,2026-05-20,,false,2,home|errands"));
        assert!(content.contains("t2,Done thing,,,,,true,0,"));
    }

    #[test]
    fn quotes_titles_with_commas() {
        let dir = tempdir().unwrap();
        let t = sample("c1", "Hello, world", json!({}));
        let res = TaskCsvConverter.convert(&[t], "csv", dir.path());
        assert!(res.errors.is_empty());
        let content = std::fs::read_to_string(&res.files_written[0]).unwrap();
        assert!(content.contains("\"Hello, world\""));
    }
}
