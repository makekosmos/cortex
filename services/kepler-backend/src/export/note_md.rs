// note_obj → markdown converter.
//
// Per-object файл: <sanitized title>.md.
// Frontmatter: YAML (id, type, created_at, updated_at, title, header_props).
// Body: TipTap JSON dom → markdown (recursive walk).

use super::{sanitize_filename, unique_path, ConvertResult, Converter};
use ark_core::types::ArkObject;
use serde_json::Value;
use std::fs;
use std::path::Path;

pub struct NoteMdConverter;

impl Converter for NoteMdConverter {
    fn id(&self) -> &'static str {
        "note_md"
    }
    fn display_name(&self) -> &'static str {
        "Заметки → Markdown"
    }
    fn object_type(&self) -> &'static str {
        "note_obj"
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
            let title_for_file = note_title(obj);
            let stem = sanitize_filename(&title_for_file);
            let path = unique_path(dest_dir, &stem, "md");

            let mut content = String::new();
            content.push_str(&render_frontmatter(obj, &title_for_file));
            content.push_str(&render_tiptap_doc(&obj.content_json, &mut result));

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
}

/// Title для имени файла: header_props.title если есть, иначе object.title.
fn note_title(obj: &ArkObject) -> String {
    if let Some(v) = obj.props_json.get("title").and_then(|v| v.as_str()) {
        if !v.trim().is_empty() {
            return v.to_string();
        }
    }
    if !obj.title.trim().is_empty() {
        return obj.title.clone();
    }
    "untitled".to_string()
}

/// YAML frontmatter. Простой подход: каждое поле на своей строке, props_json
/// инлайн как JSON (валидный YAML, т.к. JSON ⊂ YAML).
fn render_frontmatter(obj: &ArkObject, title: &str) -> String {
    let mut s = String::new();
    s.push_str("---\n");
    s.push_str(&format!("id: {}\n", yaml_scalar(&obj.id)));
    s.push_str(&format!("type: {}\n", yaml_scalar(&obj.type_id)));
    s.push_str(&format!("created_at: {}\n", yaml_scalar(&obj.created_at)));
    s.push_str(&format!("updated_at: {}\n", yaml_scalar(&obj.updated_at)));
    s.push_str(&format!("title: {}\n", yaml_scalar(title)));
    // header_props: inline JSON (JSON ⊂ YAML flow style).
    let props_str = serde_json::to_string(&obj.props_json).unwrap_or_else(|_| "{}".to_string());
    s.push_str(&format!("header_props: {props_str}\n"));
    s.push_str("---\n\n");
    s
}

fn yaml_scalar(input: &str) -> String {
    // Quote если содержит спец-символы YAML или начинается с problematic char.
    let needs_quote = input.is_empty()
        || input.contains(':')
        || input.contains('#')
        || input.contains('\n')
        || input.contains('"')
        || input.starts_with(' ')
        || input.starts_with('-')
        || input.starts_with('?')
        || input.starts_with('!')
        || input.starts_with('&')
        || input.starts_with('*')
        || input.starts_with('[')
        || input.starts_with(']')
        || input.starts_with('{')
        || input.starts_with('}');
    if needs_quote {
        let escaped = input.replace('\\', "\\\\").replace('"', "\\\"");
        format!("\"{escaped}\"")
    } else {
        input.to_string()
    }
}

/// Главный entry-point рендера TipTap dom → markdown.
pub(crate) fn render_tiptap_doc(value: &Value, result: &mut ConvertResult) -> String {
    let mut out = String::new();
    render_node(value, &mut out, result, 0);
    // Trim лишние trailing newlines, оставить ровно один в конце.
    while out.ends_with("\n\n") {
        out.pop();
    }
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn render_node(node: &Value, out: &mut String, result: &mut ConvertResult, list_depth: usize) {
    let node_type = node.get("type").and_then(|v| v.as_str()).unwrap_or("");
    match node_type {
        "doc" => {
            render_children(node, out, result, list_depth);
        }
        "paragraph" => {
            let inline = render_inline_children(node);
            out.push_str(&inline);
            out.push_str("\n\n");
        }
        "heading" => {
            let level = node
                .get("attrs")
                .and_then(|a| a.get("level"))
                .and_then(|v| v.as_u64())
                .unwrap_or(1)
                .clamp(1, 6) as usize;
            let inline = render_inline_children(node);
            out.push_str(&"#".repeat(level));
            out.push(' ');
            out.push_str(&inline);
            out.push_str("\n\n");
        }
        "bulletList" | "bullet_list" => {
            render_list(node, out, result, list_depth, false);
        }
        "orderedList" | "ordered_list" => {
            render_list(node, out, result, list_depth, true);
        }
        "listItem" | "list_item" => {
            // Обычно обрабатывается внутри render_list, но если попался напрямую —
            // рендерим как bullet item.
            let bullet = "  ".repeat(list_depth) + "- ";
            let mut inner = String::new();
            render_children(node, &mut inner, result, list_depth + 1);
            for line in inner.lines() {
                out.push_str(&bullet);
                out.push_str(line);
                out.push('\n');
            }
        }
        "codeBlock" | "code_block" => {
            let lang = node
                .get("attrs")
                .and_then(|a| a.get("language"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let text = collect_text(node);
            out.push_str("```");
            out.push_str(lang);
            out.push('\n');
            out.push_str(&text);
            if !text.ends_with('\n') {
                out.push('\n');
            }
            out.push_str("```\n\n");
        }
        "blockquote" => {
            let mut inner = String::new();
            render_children(node, &mut inner, result, list_depth);
            for line in inner.trim_end().lines() {
                out.push_str("> ");
                out.push_str(line);
                out.push('\n');
            }
            out.push('\n');
        }
        "horizontalRule" | "horizontal_rule" => {
            out.push_str("---\n\n");
        }
        "hardBreak" | "hard_break" => {
            out.push_str("  \n");
        }
        "text" => {
            out.push_str(&render_text_node(node));
        }
        "" => {
            // Корневой объект без type — попробуем как doc.
            render_children(node, out, result, list_depth);
        }
        other => {
            result.push_error(format!("unknown TipTap node type: {other}"));
            // Best-effort: рендерим children.
            render_children(node, out, result, list_depth);
        }
    }
}

fn render_children(node: &Value, out: &mut String, result: &mut ConvertResult, list_depth: usize) {
    if let Some(arr) = node.get("content").and_then(|v| v.as_array()) {
        for child in arr {
            render_node(child, out, result, list_depth);
        }
    }
}

fn render_list(
    node: &Value,
    out: &mut String,
    result: &mut ConvertResult,
    list_depth: usize,
    ordered: bool,
) {
    let indent = "  ".repeat(list_depth);
    if let Some(items) = node.get("content").and_then(|v| v.as_array()) {
        for (idx, item) in items.iter().enumerate() {
            let prefix = if ordered {
                format!("{indent}{}. ", idx + 1)
            } else {
                format!("{indent}- ")
            };
            let mut inner = String::new();
            render_children(item, &mut inner, result, list_depth + 1);
            let trimmed = inner.trim_end_matches('\n');
            let mut lines = trimmed.lines();
            if let Some(first) = lines.next() {
                out.push_str(&prefix);
                out.push_str(first);
                out.push('\n');
            } else {
                out.push_str(&prefix);
                out.push('\n');
            }
            let continuation_indent = "  ".repeat(list_depth + 1);
            for line in lines {
                if line.is_empty() {
                    out.push('\n');
                } else {
                    out.push_str(&continuation_indent);
                    out.push_str(line);
                    out.push('\n');
                }
            }
        }
        out.push('\n');
    }
}

fn render_inline_children(node: &Value) -> String {
    let mut s = String::new();
    if let Some(arr) = node.get("content").and_then(|v| v.as_array()) {
        for child in arr {
            let child_type = child.get("type").and_then(|v| v.as_str()).unwrap_or("");
            match child_type {
                "text" => s.push_str(&render_text_node(child)),
                "hardBreak" | "hard_break" => s.push_str("  \n"),
                _ => {
                    // Inline image / link / mention — best effort: текст узла.
                    s.push_str(&collect_text(child));
                }
            }
        }
    }
    s
}

fn render_text_node(node: &Value) -> String {
    let text = node.get("text").and_then(|v| v.as_str()).unwrap_or("");
    let marks = node.get("marks").and_then(|v| v.as_array());
    let mut out = text.to_string();
    if let Some(marks) = marks {
        // Apply marks от внутреннего к внешнему: code → bold/italic → link.
        let mut has_code = false;
        let mut has_bold = false;
        let mut has_italic = false;
        let mut link_href: Option<String> = None;
        for m in marks {
            let mt = m.get("type").and_then(|v| v.as_str()).unwrap_or("");
            match mt {
                "code" => has_code = true,
                "bold" | "strong" => has_bold = true,
                "italic" | "em" => has_italic = true,
                "link" => {
                    link_href = m
                        .get("attrs")
                        .and_then(|a| a.get("href"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                }
                _ => {}
            }
        }
        if has_code {
            out = format!("`{out}`");
        }
        if has_bold {
            out = format!("**{out}**");
        }
        if has_italic {
            out = format!("*{out}*");
        }
        if let Some(href) = link_href {
            out = format!("[{out}]({href})");
        }
    }
    out
}

fn collect_text(node: &Value) -> String {
    let mut s = String::new();
    if let Some(text) = node.get("text").and_then(|v| v.as_str()) {
        s.push_str(text);
    }
    if let Some(arr) = node.get("content").and_then(|v| v.as_array()) {
        for child in arr {
            s.push_str(&collect_text(child));
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    fn sample_note(id: &str, title: &str, body: Value) -> ArkObject {
        ArkObject {
            id: id.to_string(),
            type_id: "note_obj".to_string(),
            title: title.to_string(),
            content_json: body,
            props_json: json!({ "title": title }),
            created_at: "2026-05-18T10:00:00Z".to_string(),
            updated_at: "2026-05-18T10:30:00Z".to_string(),
            deleted_at: None,
        }
    }

    #[test]
    fn renders_two_notes_into_separate_files() {
        let dir = tempdir().unwrap();
        let n1 = sample_note(
            "n1",
            "First Note",
            json!({
                "type": "doc",
                "content": [
                    { "type": "heading", "attrs": { "level": 1 },
                      "content": [{ "type": "text", "text": "Hello" }] },
                    { "type": "paragraph",
                      "content": [{ "type": "text", "text": "World" }] }
                ]
            }),
        );
        let n2 = sample_note(
            "n2",
            "Second",
            json!({
                "type": "doc",
                "content": [
                    { "type": "paragraph",
                      "content": [{ "type": "text", "text": "Bullet test" }] },
                    { "type": "bulletList", "content": [
                        { "type": "listItem", "content": [
                            { "type": "paragraph",
                              "content": [{ "type": "text", "text": "a" }] }
                        ]},
                        { "type": "listItem", "content": [
                            { "type": "paragraph",
                              "content": [{ "type": "text", "text": "b" }] }
                        ]}
                    ]}
                ]
            }),
        );
        let res = NoteMdConverter.convert(&[n1, n2], "md", dir.path());
        assert!(res.errors.is_empty(), "errors: {:?}", res.errors);
        assert_eq!(res.files_written.len(), 2);

        let f1 = std::fs::read_to_string(&res.files_written[0]).unwrap();
        assert!(f1.starts_with("---\n"), "frontmatter missing: {f1}");
        assert!(f1.contains("id: n1"));
        assert!(f1.contains("# Hello"));
        assert!(f1.contains("World"));

        let f2 = std::fs::read_to_string(&res.files_written[1]).unwrap();
        assert!(f2.contains("- a"));
        assert!(f2.contains("- b"));
    }

    #[test]
    fn handles_marks_and_code_block() {
        let dir = tempdir().unwrap();
        let note = sample_note(
            "n3",
            "Marks",
            json!({
                "type": "doc",
                "content": [
                    { "type": "paragraph", "content": [
                        { "type": "text", "text": "bold", "marks": [{"type":"bold"}] },
                        { "type": "text", "text": " and " },
                        { "type": "text", "text": "italic", "marks": [{"type":"italic"}] }
                    ]},
                    { "type": "codeBlock", "attrs": { "language": "rust" }, "content": [
                        { "type": "text", "text": "fn main() {}" }
                    ]}
                ]
            }),
        );
        let res = NoteMdConverter.convert(&[note], "md", dir.path());
        assert!(res.errors.is_empty());
        let content = std::fs::read_to_string(&res.files_written[0]).unwrap();
        assert!(content.contains("**bold**"));
        assert!(content.contains("*italic*"));
        assert!(content.contains("```rust"));
        assert!(content.contains("fn main() {}"));
    }

    #[test]
    fn filename_collision_appends_suffix() {
        let dir = tempdir().unwrap();
        let n1 = sample_note("a", "Same Title", json!({ "type": "doc", "content": [] }));
        let n2 = sample_note("b", "Same Title", json!({ "type": "doc", "content": [] }));
        let res = NoteMdConverter.convert(&[n1, n2], "md", dir.path());
        assert_eq!(res.files_written.len(), 2);
        let names: Vec<String> = res
            .files_written
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert!(names.contains(&"Same Title.md".to_string()));
        assert!(names.contains(&"Same Title-2.md".to_string()));
    }

    #[test]
    fn skips_deleted_notes() {
        let dir = tempdir().unwrap();
        let mut n = sample_note("d1", "Gone", json!({ "type": "doc", "content": [] }));
        n.deleted_at = Some("2026-05-18T11:00:00Z".to_string());
        let res = NoteMdConverter.convert(&[n], "md", dir.path());
        assert!(res.files_written.is_empty());
    }
}
