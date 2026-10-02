// task_obj → markdown. Один файл на задачу.
// Frontmatter: id, type, project_id, area_id, scheduled_date, deadline, priority, completed, tags.
// Body: notes (если есть) + checklist в виде `- [ ] item` / `- [x] item`.

use super::{
    sanitize_filename, unique_path, validate_envelopes, CanonicalEnvelope, ConvertResult, Converter,
};
use ark_core::types::ArkObject;
use serde_json::Value;
use std::fs;
use std::path::Path;

pub struct TaskMdConverter;

impl Converter for TaskMdConverter {
    fn id(&self) -> &'static str {
        "task_md"
    }
    fn display_name(&self) -> &'static str {
        "Задачи → Markdown"
    }
    fn object_type(&self) -> &'static str {
        "com.kosmos.task"
    }
    fn default_format(&self) -> &'static str {
        "md"
    }
    fn supported_formats(&self) -> &'static [&'static str] {
        &["md"]
    }

    fn convert(&self, objects: &[ArkObject], _format: &str, dest_dir: &Path) -> ConvertResult {
        let mut result = ConvertResult::default();
        for obj in objects {
            if obj.deleted_at.is_some() {
                continue;
            }
            let title = if obj.title.trim().is_empty() {
                "untitled".to_string()
            } else {
                obj.title.clone()
            };
            let stem = sanitize_filename(&title);
            let path = unique_path(dest_dir, &stem, "md");

            let mut content = String::new();
            content.push_str(&render_task_frontmatter(obj, &title));
            content.push('\n');
            content.push_str(&render_task_body(obj));

            match fs::write(&path, content.as_bytes()) {
                Ok(()) => {
                    let bytes = content.len() as u64;
                    result.push_file(path, bytes);
                }
                Err(e) => result.push_error(format!("write {path:?}: {e}")),
            }
        }
        result
    }

    fn convert_canonical(
        &self,
        envelopes: &[CanonicalEnvelope],
        _format: &str,
        dest_dir: &Path,
    ) -> ConvertResult {
        let mut result = ConvertResult::default();
        let envelopes = match validate_envelopes(envelopes) {
            Ok(v) => v,
            Err(e) => {
                result.push_error(e);
                return result;
            }
        };
        for e in envelopes
            .into_iter()
            .filter(|e| e.object.deleted_at.is_none())
        {
            let o = e.object;
            let p = o.props_json;
            let title = if o.title.is_empty() {
                "untitled"
            } else {
                &o.title
            };
            let path = unique_path(dest_dir, &sanitize_filename(title), "md");
            let tags = e
                .links
                .iter()
                .filter(|l| l.link_type == "tag")
                .map(|l| l.target_object_id.clone())
                .collect::<Vec<_>>();
            let mut out = format!(
                "---\nid: {}\ntype: {}\ntypeVersion: {}\ntitle: {}\nstatus: {}\npriority:\
                     {}\nscheduledAt: {}\ndueAt: {}\ncompletedAt: {}\ncanceledAt: {}\ntags:\
                     [{}]\n---\n\n",
                yaml(&o.id),
                yaml(&o.type_id),
                yaml(&o.type_version),
                yaml(title),
                yaml(p.get("status").and_then(|v| v.as_str()).unwrap_or("inbox")),
                yaml(p.get("priority").and_then(|v| v.as_str()).unwrap_or("none")),
                yaml(
                    &p.get("scheduledAt")
                        .and_then(ark_core::canonical_types::normalize::day_value)
                        .unwrap_or_default()
                ),
                yaml(
                    &p.get("dueAt")
                        .and_then(ark_core::canonical_types::normalize::day_value)
                        .unwrap_or_default()
                ),
                yaml(
                    &p.get("completedAt")
                        .map(ToString::to_string)
                        .unwrap_or_default()
                ),
                yaml(
                    &p.get("canceledAt")
                        .map(ToString::to_string)
                        .unwrap_or_default()
                ),
                tags.iter().map(|v| yaml(v)).collect::<Vec<_>>().join(", ")
            );
            out.push_str(&collect_doc_text(&o.content_json));
            out.push('\n');
            if let Some(items) = p.get("checklist").and_then(|v| v.as_array()) {
                for item in items {
                    let done = item
                        .get("isCompleted")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    out.push_str(&format!(
                        "- [{}] {}\n",
                        if done { "x" } else { " " },
                        item.get("title").and_then(|v| v.as_str()).unwrap_or("")
                    ));
                }
            }
            match fs::write(&path, out.as_bytes()) {
                Ok(()) => result.push_file(path, out.len() as u64),
                Err(err) => result.push_error(err.to_string()),
            }
        }
        result
    }
}

fn render_task_frontmatter(obj: &ArkObject, title: &str) -> String {
    let p = &obj.props_json;
    let mut s = String::new();
    s.push_str("---\n");
    s.push_str(&format!("id: {}\n", yaml(&obj.id)));
    s.push_str(&format!("type: {}\n", yaml(&obj.type_id)));
    s.push_str(&format!("title: {}\n", yaml(title)));
    if let Some(v) = p.get("project_id").and_then(|v| v.as_str()) {
        s.push_str(&format!("project_id: {}\n", yaml(v)));
    }
    if let Some(v) = p.get("area_id").and_then(|v| v.as_str()) {
        s.push_str(&format!("area_id: {}\n", yaml(v)));
    }
    if let Some(v) = p.get("scheduled_date").and_then(|v| v.as_str()) {
        s.push_str(&format!("scheduled_date: {}\n", yaml(v)));
    }
    if let Some(v) = p.get("deadline").and_then(|v| v.as_str()) {
        s.push_str(&format!("deadline: {}\n", yaml(v)));
    }
    if let Some(v) = p.get("priority") {
        s.push_str(&format!("priority: {}\n", v));
    }
    let completed = p
        .get("completed")
        .and_then(|v| v.as_bool())
        .or_else(|| p.get("is_completed").and_then(|v| v.as_bool()))
        .unwrap_or(false);
    s.push_str(&format!("completed: {completed}\n"));
    if let Some(tags) = p.get("tags").and_then(|v| v.as_array()) {
        let joined: Vec<String> = tags
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();
        s.push_str(&format!(
            "tags: [{}]\n",
            joined
                .iter()
                .map(|t| yaml(t))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    s.push_str(&format!("created_at: {}\n", yaml(&obj.created_at)));
    s.push_str(&format!("updated_at: {}\n", yaml(&obj.updated_at)));
    s.push_str("---\n");
    s
}

fn render_task_body(obj: &ArkObject) -> String {
    let mut s = String::new();
    // Notes — может быть в props_json.notes (plain string) или в content_json как TipTap.
    if let Some(notes) = obj.props_json.get("notes").and_then(|v| v.as_str()) {
        if !notes.trim().is_empty() {
            s.push_str(notes);
            s.push_str("\n\n");
        }
    } else {
        // Попробуем content_json как plain text fallback (используем text-сбор).
        let text = collect_doc_text(&obj.content_json);
        if !text.trim().is_empty() {
            s.push_str(&text);
            s.push_str("\n\n");
        }
    }
    // Checklist items.
    let checklist = obj
        .props_json
        .get("checklist_items")
        .or_else(|| obj.props_json.get("checklist"))
        .and_then(|v| v.as_array());
    if let Some(items) = checklist {
        if !items.is_empty() {
            s.push_str("## Чек-лист\n\n");
            for item in items {
                let title = item
                    .get("title")
                    .or_else(|| item.get("text"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let done = item
                    .get("completed")
                    .or_else(|| item.get("is_completed"))
                    .or_else(|| item.get("done"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let mark = if done { "x" } else { " " };
                s.push_str(&format!("- [{mark}] {title}\n"));
            }
            s.push('\n');
        }
    }
    s
}

fn collect_doc_text(value: &Value) -> String {
    let mut s = String::new();
    if let Some(text) = value.get("text").and_then(|v| v.as_str()) {
        s.push_str(text);
        s.push(' ');
    }
    if let Some(arr) = value.get("content").and_then(|v| v.as_array()) {
        for child in arr {
            s.push_str(&collect_doc_text(child));
        }
    }
    s
}

fn yaml(input: &str) -> String {
    let needs_quote = input.is_empty()
        || input.contains(':')
        || input.contains('#')
        || input.contains('\n')
        || input.contains('"')
        || input.starts_with(' ')
        || input.starts_with('-');
    if needs_quote {
        let escaped = input.replace('\\', "\\\\").replace('"', "\\\"");
        format!("\"{escaped}\"")
    } else {
        input.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    fn sample_task(id: &str, title: &str, props: Value) -> ArkObject {
        ArkObject {
            id: id.to_string(),
            type_id: "task_obj".to_string(),
            type_version: "0.0.0-legacy".to_string(),
            title: title.to_string(),
            content_json: json!({ "type": "doc", "content": [] }),
            props_json: props,
            created_at: "2026-05-18T10:00:00Z".to_string(),
            updated_at: "2026-05-18T10:30:00Z".to_string(),
            deleted_at: None,
        }
    }

    #[test]
    fn writes_task_with_frontmatter_and_checklist() {
        let dir = tempdir().unwrap();
        let t = sample_task(
            "t1",
            "Buy milk",
            json!({
                "project_id": "p1",
                "scheduled_date": "2026-05-20",
                "priority": 2,
                "completed": false,
                "tags": ["home", "errands"],
                "notes": "remember oat",
                "checklist_items": [
                    { "title": "2%", "completed": true },
                    { "title": "skim", "completed": false }
                ]
            }),
        );
        let res = TaskMdConverter.convert(&[t], "md", dir.path());
        assert!(res.errors.is_empty(), "errors: {:?}", res.errors);
        assert_eq!(res.files_written.len(), 1);
        let content = std::fs::read_to_string(&res.files_written[0]).unwrap();
        assert!(content.contains("id: t1"));
        assert!(content.contains("project_id: p1"));
        assert!(content.contains("priority: 2"));
        assert!(content.contains("completed: false"));
        assert!(content.contains("tags: [home, errands]"));
        assert!(content.contains("remember oat"));
        assert!(content.contains("## Чек-лист"));
        assert!(content.contains("- [x] 2%"));
        assert!(content.contains("- [ ] skim"));
    }

    #[test]
    fn writes_minimal_task_without_optional_fields() {
        let dir = tempdir().unwrap();
        let t = sample_task("t2", "Just a task", json!({}));
        let res = TaskMdConverter.convert(&[t], "md", dir.path());
        assert!(res.errors.is_empty());
        assert_eq!(res.files_written.len(), 1);
        let content = std::fs::read_to_string(&res.files_written[0]).unwrap();
        assert!(content.contains("id: t2"));
        assert!(content.contains("completed: false"));
        assert!(!content.contains("project_id"));
    }
}
