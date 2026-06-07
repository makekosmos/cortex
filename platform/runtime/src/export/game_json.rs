// game_obj → JSON. Один файл games.json (pretty-printed массив объектов).

use super::{ConvertResult, Converter};
use ark_core::types::ArkObject;
use std::fs;
use std::path::Path;

pub struct GameJsonConverter;

impl Converter for GameJsonConverter {
    fn id(&self) -> &'static str {
        "game_json"
    }
    fn display_name(&self) -> &'static str {
        "Игры → JSON"
    }
    fn object_type(&self) -> &'static str {
        "game_obj"
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
        let path = dest_dir.join("games.json");
        match serde_json::to_vec_pretty(&alive) {
            Ok(bytes) => match fs::write(&path, &bytes) {
                Ok(()) => result.push_file(path, bytes.len() as u64),
                Err(e) => result.push_error(format!("write {path:?}: {e}")),
            },
            Err(e) => result.push_error(format!("serialize games: {e}")),
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
            type_id: "game_obj".to_string(),
            title: title.to_string(),
            content_json: json!({}),
            props_json: props,
            created_at: "2026-05-18T10:00:00Z".to_string(),
            updated_at: "2026-05-18T10:30:00Z".to_string(),
            deleted_at: None,
        }
    }

    #[test]
    fn writes_games_json_array() {
        let dir = tempdir().unwrap();
        let g1 = sample("g1", "Game One", json!({ "platform": "steam" }));
        let g2 = sample("g2", "Game Two", json!({ "platform": "epic" }));
        let res = GameJsonConverter.convert(&[g1, g2], "json", dir.path());
        assert!(res.errors.is_empty(), "errors: {:?}", res.errors);
        assert_eq!(res.files_written.len(), 1);
        let content = std::fs::read_to_string(&res.files_written[0]).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed.as_array().unwrap().len(), 2);
        assert_eq!(parsed[0]["title"], "Game One");
    }

    #[test]
    fn excludes_deleted_games() {
        let dir = tempdir().unwrap();
        let alive = sample("a", "live", json!({}));
        let mut dead = sample("d", "deleted", json!({}));
        dead.deleted_at = Some("2026-05-18T11:00:00Z".to_string());
        let res = GameJsonConverter.convert(&[alive, dead], "json", dir.path());
        assert!(res.errors.is_empty());
        let content = std::fs::read_to_string(&res.files_written[0]).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed.as_array().unwrap().len(), 1);
    }
}
