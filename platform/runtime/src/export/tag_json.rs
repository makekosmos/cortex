// tag_obj → JSON. Один файл tags.json (pretty-printed массив объектов).

use super::{ConvertResult, Converter};
use ark_core::types::ArkObject;
use std::fs;
use std::path::Path;

pub struct TagJsonConverter;

impl Converter for TagJsonConverter {
    fn id(&self) -> &'static str {
        "tag_json"
    }
    fn display_name(&self) -> &'static str {
        "Теги → JSON"
    }
    fn object_type(&self) -> &'static str {
        "tag_obj"
    }
    fn default_format(&self) -> &'static str {
        "json"
    }
    fn supported_formats(&self) -> &'static [&'static str] {
        &["json"]
    }

    fn convert(&self, objects: &[ArkObject], _format: &str, dest_dir: &Path) -> ConvertResult {
        let mut result = ConvertResult::default();
        let alive: Vec<&ArkObject> = objects.iter().filter(|o| o.deleted_at.is_none()).collect();
        let path = dest_dir.join("tags.json");
        match serde_json::to_vec_pretty(&alive) {
            Ok(bytes) => match fs::write(&path, &bytes) {
                Ok(()) => result.push_file(path, bytes.len() as u64),
                Err(e) => result.push_error(format!("write {path:?}: {e}")),
            },
            Err(e) => result.push_error(format!("serialize tags: {e}")),
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
            type_id: "tag_obj".to_string(),
            title: title.to_string(),
            content_json: json!({}),
            props_json: props,
            created_at: "2026-05-18T10:00:00Z".to_string(),
            updated_at: "2026-05-18T10:30:00Z".to_string(),
            deleted_at: None,
        }
    }

    #[test]
    fn writes_tags_json_array() {
        let dir = tempdir().unwrap();
        let a = sample("tag1", "work", json!({ "color": "#ff0000" }));
        let b = sample("tag2", "home", json!({ "color": "#00ff00" }));
        let res = TagJsonConverter.convert(&[a, b], "json", dir.path());
        assert!(res.errors.is_empty(), "errors: {:?}", res.errors);
        assert_eq!(res.files_written.len(), 1);
        let content = std::fs::read_to_string(&res.files_written[0]).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        let arr = parsed.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["id"], "tag1");
        assert_eq!(arr[1]["title"], "home");
    }

    #[test]
    fn excludes_deleted_tags() {
        let dir = tempdir().unwrap();
        let alive = sample("a", "live", json!({}));
        let mut dead = sample("d", "deleted", json!({}));
        dead.deleted_at = Some("2026-05-18T11:00:00Z".to_string());
        let res = TagJsonConverter.convert(&[alive, dead], "json", dir.path());
        assert!(res.errors.is_empty());
        let content = std::fs::read_to_string(&res.files_written[0]).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed.as_array().unwrap().len(), 1);
    }
}
