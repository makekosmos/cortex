use serde::{Deserialize, Serialize};
use serde_yaml::Value as YamlValue;
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use flate2::read::GzDecoder;
use lazy_static::lazy_static;
use regex::Regex;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SqobaManifest {
    pub games: HashMap<String, SqobaGame>,

    // Derived data for fast lookups; never serialized.
    #[serde(skip)]
    index: SqobaManifestIndex,
}

#[derive(Debug, Clone, Default)]
struct SqobaManifestIndex {
    normalized_keys: Vec<(String, String)>,
    normalized_exact: HashMap<String, String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SqobaGame {
    pub files: Option<HashMap<String, Vec<String>>>,
    pub registry: Option<Vec<String>>,
}

impl SqobaManifest {
    pub(crate) fn from_games(games: HashMap<String, SqobaGame>) -> Self {
        let mut manifest = Self {
            games,
            index: SqobaManifestIndex::default(),
        };
        manifest.rebuild_index();
        manifest
    }

    fn rebuild_index(&mut self) {
        self.index.normalized_keys.clear();
        self.index.normalized_exact.clear();
        self.index.normalized_keys.reserve(self.games.len());

        for key in self.games.keys() {
            let normalized = normalize_name(key);
            self.index
                .normalized_exact
                .entry(normalized.clone())
                .or_insert_with(|| key.clone());
            self.index.normalized_keys.push((key.clone(), normalized));
        }
    }

    pub fn find_game_entry(&self, name: &str) -> Option<(String, SqobaGame)> {
        if let Some(entry) = self.games.get(name) {
            return Some((name.to_string(), entry.clone()));
        }

        let normalized = normalize_name(name);

        if let Some(key) = self.index.normalized_exact.get(&normalized) {
            return self
                .games
                .get(key)
                .cloned()
                .map(|entry| (key.clone(), entry));
        }

        let mut best: Option<(String, f32)> = None;
        for (key, key_norm) in &self.index.normalized_keys {
            let score = similarity_score(&normalized, key_norm);
            if best.as_ref().map(|b| score > b.1).unwrap_or(true) {
                best = Some((key.clone(), score));
            }
        }

        if let Some((best_key, best_score)) = best {
            if best_score >= 0.6 {
                return self
                    .games
                    .get(&best_key)
                    .cloned()
                    .map(|entry| (best_key, entry));
            }
        }

        None
    }

    pub fn suggest_games(&self, name: &str, limit: usize) -> Vec<String> {
        let normalized = normalize_name(name);
        let mut scored: Vec<(String, f32)> = self
            .index
            .normalized_keys
            .iter()
            .map(|(key, key_norm)| (key.clone(), similarity_score(&normalized, key_norm)))
            .filter(|(_, score)| *score >= 0.4)
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().take(limit).map(|(k, _)| k).collect()
    }
}

const CACHE_FILE_NAME: &str = "sqoba_manifest.json";
const EMBEDDED_MANIFEST_GZ: &[u8] = include_bytes!("../../resources/sqoba_manifest.yaml.gz");

lazy_static! {
    static ref NORMALIZE_RE: Regex = Regex::new(r"[^a-z0-9]+").expect("regex for normalize_name");
}

#[allow(dead_code)]
pub fn load_manifest() -> Result<SqobaManifest, String> {
    let manifest = load_manifest_optional()?;
    manifest.ok_or_else(|| "Манифест SQOBA не найден (кэш пуст и загрузка не удалась)".to_string())
}

pub fn load_manifest_optional() -> Result<Option<SqobaManifest>, String> {
    let cache_path = default_cache_path();
    load_manifest_optional_with_cache_and_fetcher(&cache_path, download_ludusavi_manifest_yaml)
}

fn default_cache_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("arrancador")
        .join(CACHE_FILE_NAME)
}

fn load_manifest_from_cache(cache_path: &Path) -> Option<SqobaManifest> {
    if !cache_path.exists() {
        return None;
    }

    let file = File::open(cache_path).ok()?;
    let reader = std::io::BufReader::new(file);
    let mut manifest: SqobaManifest = serde_json::from_reader(reader).ok()?;
    manifest.rebuild_index();
    Some(manifest)
}

fn write_manifest_cache(cache_path: &Path, manifest: &SqobaManifest) -> Result<(), String> {
    if let Some(parent) = cache_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_vec(manifest).map_err(|e| e.to_string())?;
    fs::write(cache_path, json).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn refresh_manifest_from_network() -> Result<(), String> {
    let cache_path = default_cache_path();
    let Some(text) = download_ludusavi_manifest_yaml()? else {
        return Err("Не удалось скачать манифест".to_string());
    };

    let manifest = manifest_from_yaml(&text)?;
    write_manifest_cache(&cache_path, &manifest)?;
    Ok(())
}

fn load_manifest_optional_with_cache_and_fetcher<F>(
    cache_path: &Path,
    fetcher: F,
) -> Result<Option<SqobaManifest>, String>
where
    F: FnOnce() -> Result<Option<String>, String>,
{
    if let Some(manifest) = load_manifest_from_cache(cache_path) {
        return Ok(Some(manifest));
    }

    if let Some(text) = fetcher()? {
        let manifest = manifest_from_yaml(&text)?;
        write_manifest_cache(cache_path, &manifest)?;
        return Ok(Some(manifest));
    }

    let Some(text) = load_embedded_manifest_yaml() else {
        return Ok(None);
    };

    let manifest = manifest_from_yaml(&text)?;
    write_manifest_cache(cache_path, &manifest)?;
    Ok(Some(manifest))
}

fn download_ludusavi_manifest_yaml() -> Result<Option<String>, String> {
    // We cache the parsed manifest, so this should run rarely (only when cache is missing).
    // Try both default branch names to be resilient to repo changes.
    const URLS: [&str; 2] = [
        "https://raw.githubusercontent.com/mtkennerly/ludusavi-manifest/main/data/manifest.yaml",
        "https://raw.githubusercontent.com/mtkennerly/ludusavi-manifest/master/data/manifest.yaml",
    ];

    let client = match reqwest::blocking::Client::builder()
        .user_agent("arrancador (SQOBA)")
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(15))
        .build()
    {
        Ok(client) => client,
        Err(_) => return Ok(None),
    };

    for url in URLS {
        let resp = match client.get(url).send() {
            Ok(r) => r,
            Err(_) => continue,
        };

        if !resp.status().is_success() {
            continue;
        }

        let text = match resp.text() {
            Ok(text) => text,
            Err(_) => continue,
        };
        if text.trim().is_empty() {
            continue;
        }

        return Ok(Some(text));
    }

    Ok(None)
}

fn load_embedded_manifest_yaml() -> Option<String> {
    if EMBEDDED_MANIFEST_GZ.is_empty() {
        return None;
    }

    let mut decoder = GzDecoder::new(EMBEDDED_MANIFEST_GZ);
    let mut text = String::new();
    if decoder.read_to_string(&mut text).is_err() {
        return None;
    }

    if text.trim().is_empty() {
        None
    } else {
        Some(text)
    }
}

fn manifest_from_yaml(text: &str) -> Result<SqobaManifest, String> {
    let root: YamlValue = serde_yaml::from_str(text).map_err(|e| e.to_string())?;
    let mapping = root
        .as_mapping()
        .ok_or_else(|| "Неверный формат манифеста".to_string())?;

    let mut games: HashMap<String, SqobaGame> = HashMap::new();

    for (game_name, game_val) in mapping {
        let name = match game_name.as_str() {
            Some(n) => n.to_string(),
            None => continue,
        };

        let mut files_map: HashMap<String, Vec<String>> = HashMap::new();
        if let Some(files) = game_val
            .as_mapping()
            .and_then(|m| m.get(YamlValue::from("files")))
            .and_then(|v| v.as_mapping())
        {
            for (path_key, meta_val) in files {
                let path = match path_key.as_str() {
                    Some(p) => p.to_string(),
                    None => continue,
                };
                if !is_path_applicable(meta_val) {
                    continue;
                }
                let tags = extract_tags(meta_val);
                for tag in tags {
                    files_map.entry(tag).or_default().push(path.clone());
                }
            }
        }

        let game_manifest = SqobaGame {
            files: if files_map.is_empty() {
                None
            } else {
                Some(files_map)
            },
            registry: None,
        };
        games.insert(name, game_manifest);
    }

    Ok(SqobaManifest::from_games(games))
}

fn extract_tags(meta: &YamlValue) -> Vec<String> {
    if let Some(tags) = meta
        .as_mapping()
        .and_then(|m| m.get(YamlValue::from("tags")))
        .and_then(|v| v.as_sequence())
    {
        let mut out = Vec::new();
        for t in tags {
            if let Some(s) = t.as_str() {
                out.push(s.to_string());
            }
        }
        if !out.is_empty() {
            return out;
        }
    }
    vec!["save".to_string()]
}

fn is_path_applicable(meta: &YamlValue) -> bool {
    let when = meta
        .as_mapping()
        .and_then(|m| m.get(YamlValue::from("when")))
        .and_then(|v| v.as_sequence());
    if when.is_none() {
        return true;
    }

    for cond in when.unwrap() {
        if let Some(map) = cond.as_mapping() {
            if let Some(os_val) = map.get(YamlValue::from("os")).and_then(|v| v.as_str()) {
                let os = os_val.to_lowercase();
                if os == "windows" || os == "win" {
                    return true;
                } else {
                    continue;
                }
            } else {
                return true;
            }
        }
    }
    false
}

pub fn normalize_name(name: &str) -> String {
    let lower = name.to_lowercase();
    let cleaned = NORMALIZE_RE.replace_all(&lower, " ");
    let stop_words = [
        "the",
        "a",
        "an",
        "edition",
        "definitive",
        "remastered",
        "goty",
        "game",
        "of",
        "year",
        "ultimate",
        "complete",
        "collection",
        "bundle",
        "deluxe",
        "enhanced",
        "hd",
    ];
    let tokens: Vec<&str> = cleaned
        .split_whitespace()
        .filter(|t| !stop_words.contains(t))
        .collect();
    tokens.join(" ")
}

pub fn similarity_score(a: &str, b: &str) -> f32 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    if a == b {
        return 1.0;
    }
    if a.contains(b) || b.contains(a) {
        return 0.9;
    }
    let set_a: HashSet<&str> = a.split_whitespace().collect();
    let set_b: HashSet<&str> = b.split_whitespace().collect();
    if set_a.is_empty() || set_b.is_empty() {
        return 0.0;
    }
    let inter = set_a.intersection(&set_b).count() as f32;
    let union = set_a.union(&set_b).count() as f32;
    inter / union
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::tempdir;

    #[test]
    fn load_manifest_prefers_cache_when_present() {
        let dir = tempdir().expect("tempdir");
        let cache_path = dir.path().join("cache.json");

        let mut games = HashMap::new();
        games.insert(
            "Cached Game".to_string(),
            SqobaGame {
                files: None,
                registry: None,
            },
        );
        let manifest = SqobaManifest::from_games(games);
        let json = serde_json::to_string(&manifest).expect("serialize manifest");
        fs::write(&cache_path, json).expect("write cache");

        let loaded = load_manifest_optional_with_cache_and_fetcher(&cache_path, || {
            panic!("cache should be used, fetcher must not run")
        })
        .expect("load manifest")
        .expect("manifest present");

        assert!(loaded.games.contains_key("Cached Game"));
    }

    #[test]
    fn load_manifest_from_fetcher_writes_cache() {
        let dir = tempdir().expect("tempdir");
        let cache_path = dir.path().join("cache.json");

        let yaml = r#"
Example Game:
  files:
    "<winLocalAppData>/Example/save.dat":
      tags: ["save"]
"#
        .to_string();

        let loaded = load_manifest_optional_with_cache_and_fetcher(&cache_path, || Ok(Some(yaml)))
            .expect("load manifest")
            .expect("manifest present");

        assert!(loaded.games.contains_key("Example Game"));
        assert!(cache_path.exists());
    }

    #[test]
    fn find_game_entry_matches_normalized_name() {
        let mut games = HashMap::new();
        games.insert(
            "The Witcher 3: Game of the Year Edition".to_string(),
            SqobaGame {
                files: None,
                registry: None,
            },
        );
        let manifest = SqobaManifest::from_games(games);

        let found = manifest.find_game_entry("witcher 3").expect("find game");
        assert_eq!(found.0, "The Witcher 3: Game of the Year Edition");
    }
}
