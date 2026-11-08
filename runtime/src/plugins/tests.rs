//! The store: manifests kept beside installs, and settings kept where the
//! plugin says.

use serde_json::{json, Map};

use super::manifest::PluginManifest;
use super::{PluginError, PluginStore};

fn store(dir: &tempfile::TempDir) -> PluginStore {
    PluginStore::at(dir.path().join("Apps"), dir.path().join("data")).expect("store")
}

fn manifest(settings_file: Option<&str>) -> PluginManifest {
    serde_json::from_value(json!({
        "schema_version": 1,
        "id": "com.kosmos.dictation",
        "name": "Диктовка",
        "source": { "kind": "release", "repository": "makekosmos/dictation",
                    "tag_prefix": "gpui-v", "asset": "dictation-gpui" },
        "settings_file": settings_file,
        "settings": [
            {
                "key": "hotkey", "label": "Клавиша",
                "kind": { "type": "text" }, "value": "Ctrl+Shift+;"
            },
            {
                "key": "provider_enabled", "label": "Провайдер",
                "kind": { "type": "toggle" }, "value": true
            }
        ]
    }))
    .expect("manifest")
}

#[test]
fn a_manifest_survives_a_round_trip_through_the_store() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(&dir);
    let parsed = manifest(None);
    store.save_manifest(&parsed).expect("saved");
    assert_eq!(store.manifest(&parsed.id), Some(parsed.clone()));
    // The file is next to where the versions install, and readable as JSON.
    let path = dir.path().join("Apps").join(&parsed.id).join("plugin.json");
    let text = std::fs::read_to_string(path).expect("plugin.json");
    assert!(text.contains("dictation-gpui"));
}

#[test]
fn a_manifest_stored_under_another_id_is_not_that_plugin() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(&dir);
    let parsed = manifest(None);
    let other = dir.path().join("Apps").join("com.kosmos.other");
    std::fs::create_dir_all(&other).unwrap();
    std::fs::write(
        other.join("plugin.json"),
        serde_json::to_vec(&parsed).unwrap(),
    )
    .unwrap();
    assert_eq!(store.manifest("com.kosmos.other"), None);
}

#[test]
fn settings_default_to_the_manifest_and_then_follow_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(&dir);
    let parsed = manifest(None);
    store.save_manifest(&parsed).expect("saved");

    let values = store.settings(&parsed).expect("defaults");
    assert_eq!(values["hotkey"], json!("Ctrl+Shift+;"));
    assert_eq!(values["provider_enabled"], json!(true));

    let mut patch = Map::new();
    patch.insert("hotkey".into(), json!("Ctrl+Alt+D"));
    let saved = store.set_settings(&parsed, &patch).expect("saved");
    assert_eq!(saved["hotkey"], json!("Ctrl+Alt+D"));
    // Written inside the plugin's own directory by default.
    let config = dir.path().join("Apps").join(&parsed.id).join("config.json");
    assert!(config.is_file());
    // And read back on the next call.
    assert_eq!(
        store.settings(&parsed).expect("stored")["hotkey"],
        json!("Ctrl+Alt+D")
    );
}

#[test]
fn a_manifest_that_names_its_own_settings_file_writes_there() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(&dir);
    let parsed = manifest(Some("dictation-config.json"));
    store.save_manifest(&parsed).expect("saved");
    let mut patch = Map::new();
    patch.insert("hotkey".into(), json!("Ctrl+Shift+Space"));
    store.set_settings(&parsed, &patch).expect("saved");
    let config = dir.path().join("data").join("dictation-config.json");
    let text = std::fs::read_to_string(&config).expect("config");
    assert!(text.contains("Ctrl+Shift+Space"));
    assert!(!dir
        .path()
        .join("Apps")
        .join(&parsed.id)
        .join("config.json")
        .exists());
}

#[test]
fn settings_the_manifest_does_not_declare_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(&dir);
    let parsed = manifest(None);
    let mut patch = Map::new();
    patch.insert("hotkey".into(), json!("Ctrl+D"));
    patch.insert("whisper_secret".into(), json!("x"));
    let error = store
        .set_settings(&parsed, &patch)
        .expect_err("unknown key");
    assert!(matches!(error, PluginError::Settings(_)));
    // Nothing was written.
    assert!(!dir
        .path()
        .join("Apps")
        .join(&parsed.id)
        .join("config.json")
        .exists());
}

#[test]
fn keys_the_file_already_has_are_kept_when_another_view_sets_one() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(&dir);
    let parsed = manifest(Some("dictation-config.json"));
    store.save_manifest(&parsed).expect("saved");
    // The Engine's own dictation settings wrote this file before plugins did.
    std::fs::create_dir_all(dir.path().join("data")).unwrap();
    std::fs::write(
        dir.path().join("data").join("dictation-config.json"),
        json!({ "model": "whisper-large-v3", "hotkey": "Ctrl+;" }).to_string(),
    )
    .unwrap();

    let mut patch = Map::new();
    patch.insert("hotkey".into(), json!("Ctrl+Alt+D"));
    store.set_settings(&parsed, &patch).expect("saved");

    let text =
        std::fs::read_to_string(dir.path().join("data").join("dictation-config.json")).unwrap();
    let stored: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(stored["hotkey"], json!("Ctrl+Alt+D"));
    assert_eq!(stored["model"], json!("whisper-large-v3"));
}

#[test]
fn a_plugin_that_is_not_installed_has_nothing_to_show() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(&dir);
    assert!(store.installed().is_empty());
    assert_eq!(store.manifest("com.kosmos.dictation"), None);
    assert_eq!(store.executable("com.kosmos.dictation"), None);
    assert!(store
        .record("com.kosmos.dictation")
        .expect("record")
        .is_none());
    // Removing what is not there is not an error.
    store.remove("com.kosmos.dictation").expect("no-op");
}

/// The catalog this repository publishes is the one the Engine will fetch:
/// it has to parse, and the dictation settings have to keep the spelling the
/// dictation config file uses (`camelCase`, its own `serde` rename).
#[test]
fn the_published_catalog_parses_and_mirrors_the_dictation_config() {
    let text = include_str!("../../../plugins/catalog.json");
    let catalog = super::catalog::Catalog::parse_for_test(text).expect("published catalog");
    let dictation = catalog
        .plugins
        .iter()
        .find(|plugin| plugin.id == "com.kosmos.dictation")
        .expect("dictation in the catalog");
    assert_eq!(
        dictation.settings_file.as_deref(),
        Some("dictation-config.json")
    );
    let keys: Vec<&str> = dictation
        .settings
        .iter()
        .map(|field| field.key.as_str())
        .collect();
    assert_eq!(
        keys,
        [
            "hotkey",
            "triggerMode",
            "language",
            "injectMode",
            "providerEnabled",
            "httpProxy",
            "transcriptionPrompt"
        ]
    );
}
