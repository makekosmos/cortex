use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReaderState {
    pub font_size: f32,
    #[serde(default = "default_font_family")]
    pub font_family: String,
    pub last_path: Option<PathBuf>,
    pub chapter_index: usize,
}

impl Default for ReaderState {
    fn default() -> Self {
        Self {
            font_size: 18.0,
            font_family: default_font_family(),
            last_path: None,
            chapter_index: 0,
        }
    }
}

fn default_font_family() -> String {
    "Georgia".to_string()
}

impl ReaderState {
    pub fn load(user_data_dir: &Path) -> Self {
        let path = user_data_dir.join("reader-state.json");
        let Ok(raw) = fs::read_to_string(path) else {
            return Self::default();
        };
        serde_json::from_str(&raw).unwrap_or_default()
    }

    pub fn save(&self, user_data_dir: &Path) -> Result<()> {
        fs::create_dir_all(user_data_dir)
            .with_context(|| format!("failed to create {}", user_data_dir.display()))?;
        let raw = serde_json::to_string_pretty(self)?;
        fs::write(user_data_dir.join("reader-state.json"), raw)
            .with_context(|| format!("failed to write state in {}", user_data_dir.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persists_reader_state() {
        let dir = tempfile::tempdir().expect("temp dir should be created");
        let state = ReaderState {
            font_size: 21.0,
            font_family: "Geist".to_string(),
            last_path: Some(PathBuf::from("book.epub")),
            chapter_index: 2,
        };

        state.save(dir.path()).expect("reader state should save");
        assert_eq!(ReaderState::load(dir.path()), state);
    }
}
