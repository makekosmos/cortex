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
    // What the dictation app's own settings page shows, in its order.
    assert_eq!(
        keys,
        [
            "hotkey",
            "providerEnabled",
            "injectMode",
            "duckAudioDuringRecording",
            "language",
            "localModel",
            "localIdleUnloadMs"
        ]
    );
    // Every key is a field of the config the app reads, and the page's edits
    // are applied by the operation that owns that config.
    let config = serde_json::to_value(crate::dictation::config::DictationConfig::default())
        .expect("config as json");
    for key in &keys {
        assert!(config.get(*key).is_some(), "{key} is not a dictation field");
    }
    assert_eq!(
        dictation.settings_op.as_deref(),
        Some("dictation.update_config")
    );
}

/// The dictation app this Engine already installs predates the catalog: it has
/// `install.json` but no `plugin.json`. The catalog says who it is, and the
/// copy is kept beside the install so the page opens once, not once per visit.
#[test]
fn an_install_without_a_manifest_takes_the_catalogs() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(&dir);
    let app = dir.path().join("Apps").join("com.kosmos.dictation");
    std::fs::create_dir_all(app.join("0.5.2")).unwrap();
    std::fs::write(
        app.join("install.json"),
        json!({
            "schema_version": 1,
            "id": "com.kosmos.dictation",
            "version": "0.5.2",
            "executable": "dictation-gpui.exe",
            "sha256": "0".repeat(64),
            "size": 4_200_000,
            "repository": "makekosmos/dictation",
            "release_tag": "gpui-v0.5.2",
            "installed_at": 1_794_000_000_000u64,
        })
        .to_string(),
    )
    .unwrap();
    std::fs::write(app.join("0.5.2").join("dictation-gpui.exe"), b"MZ").unwrap();
    assert_eq!(store.manifest("com.kosmos.dictation"), None);
    assert!(store.record("com.kosmos.dictation").unwrap().is_some());

    let catalog =
        super::catalog::Catalog::parse_for_test(include_str!("../../../plugins/catalog.json"))
            .expect("published catalog");
    let adopted = super::ops::adopt(&store, &catalog, "com.kosmos.dictation").expect("adopted");
    assert_eq!(adopted.name, "Диктовка");
    // Kept: the next read does not need the catalog.
    assert_eq!(store.manifest("com.kosmos.dictation"), Some(adopted));
    let installed = store.installed();
    assert_eq!(
        installed[0].manifest.as_ref().map(|m| m.name.as_str()),
        Some("Диктовка")
    );
    // And its settings are the ones the dictation stack already wrote.
    assert_eq!(installed[0].settings["hotkey"], json!("Ctrl+Shift+;"));
}

/// The list of plugins that start with the Engine: written on the first
/// answer, kept as answered afterwards, and never naming an uninstalled plugin
/// back to life.
#[test]
fn the_autostart_list_is_written_once_and_then_obeyed() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(&dir);
    let data = dir.path().join("data");
    assert!(!super::autostart::stored(&data));

    // First answer: an installed plugin that says how it starts goes on the list.
    let mut parsed = manifest(None);
    parsed.start = Some(super::PluginStart {
        args: vec!["--background".into()],
    });
    store.save_manifest(&parsed).expect("manifest");
    let apps = dir.path().join("Apps").join(&parsed.id);
    std::fs::create_dir_all(apps.join("0.5.2")).unwrap();
    std::fs::write(
        apps.join("install.json"),
        json!({
            "schema_version": 1,
            "id": parsed.id,
            "version": "0.5.2",
            "executable": "dictation-gpui.exe",
            "sha256": "0".repeat(64),
            "size": 4_200_000,
            "repository": "makekosmos/dictation",
            "release_tag": "gpui-v0.5.2",
            "installed_at": 1_794_000_000_000u64,
        })
        .to_string(),
    )
    .unwrap();
    std::fs::write(apps.join("0.5.2").join("dictation-gpui.exe"), b"MZ").unwrap();

    let installed = store.installed();
    assert!(installed[0].startable, "the manifest says how it starts");
    assert!(!installed[0].autostart, "nothing was written yet");
    assert_eq!(
        store.start_args(&parsed.id),
        Some(vec!["--background".to_owned()])
    );

    // Answering "off" writes the list, and a written list is not re-seeded.
    let ids = super::autostart::set(&store, &parsed.id, false).expect("off");
    assert!(ids.is_empty());
    assert!(super::autostart::stored(&data));
    assert!(!store.installed()[0].autostart);

    let ids = super::autostart::set(&store, &parsed.id, true).expect("on");
    assert_eq!(ids, vec![parsed.id.clone()]);
    assert!(store.installed()[0].autostart);
    // What it starts with is written down with it: an older catalog cannot
    // take the pill's command line away later.
    let entries = super::autostart::load(&data);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].args, ["--background"]);

    // Uninstalling the plugin takes it off the list.
    std::fs::remove_file(apps.join("0.5.2").join("dictation-gpui.exe")).ok();
    store.remove(&parsed.id).expect("removed");
    assert!(super::autostart::load(&data).is_empty());
}

#[test]
fn a_plugin_without_a_program_cannot_be_put_on_the_list() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(&dir);
    let mut parsed = manifest(None);
    parsed.start = Some(super::PluginStart::default());
    store.save_manifest(&parsed).expect("manifest");
    let error = super::autostart::set(&store, &parsed.id, true).unwrap_err();
    assert!(matches!(error, PluginError::NotInstalled), "{error:?}");
}

/// The published catalog names the command line the pill starts with.
#[test]
fn the_catalog_says_the_dictation_pill_starts_in_the_background() {
    let catalog =
        super::catalog::Catalog::parse_for_test(include_str!("../../../plugins/catalog.json"))
            .expect("published catalog");
    let dictation = catalog
        .plugins
        .iter()
        .find(|plugin| plugin.id == "com.kosmos.dictation")
        .expect("dictation in the catalog");
    let start = dictation.start.as_ref().expect("a start line");
    assert_eq!(start.args, ["--background"]);
}

/// A manifest can lag behind (an older catalog, an offline cache) — the
/// command line already answered for is not lost by switching the switch off
/// and on again.
#[test]
fn the_command_line_survives_a_manifest_that_forgot_it() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(&dir);
    let data = dir.path().join("data");
    let mut parsed = manifest(None);
    parsed.start = Some(super::PluginStart {
        args: vec!["--background".into()],
    });
    store.save_manifest(&parsed).expect("manifest");
    let apps = dir.path().join("Apps").join(&parsed.id);
    std::fs::create_dir_all(apps.join("0.5.2")).unwrap();
    std::fs::write(
        apps.join("install.json"),
        json!({
            "schema_version": 1,
            "id": parsed.id,
            "version": "0.5.2",
            "executable": "dictation-gpui.exe",
            "sha256": "0".repeat(64),
            "size": 4_200_000,
            "repository": "makekosmos/dictation",
            "release_tag": "gpui-v0.5.2",
            "installed_at": 1_794_000_000_000u64,
        })
        .to_string(),
    )
    .unwrap();
    std::fs::write(apps.join("0.5.2").join("dictation-gpui.exe"), b"MZ").unwrap();
    super::autostart::set(&store, &parsed.id, true).expect("on");

    // The catalog this machine can reach says less than the manifest did.
    parsed.start = None;
    store
        .save_manifest(&parsed)
        .expect("manifest without start");
    assert_eq!(store.start_args(&parsed.id), None);

    super::autostart::set(&store, &parsed.id, false).expect("off");
    super::autostart::set(&store, &parsed.id, true).expect("on again");
    assert_eq!(
        super::autostart::load(&data)[0].args,
        ["--background"],
        "the pill is still started the way it was answered"
    );
}
