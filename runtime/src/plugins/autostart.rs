//! Plugins that start with the Engine.
//!
//! The Engine already starts at sign-in (`<exe> --start`). A plugin whose
//! window is not a window — the dictation pill is one — has nothing to gain
//! from being launched by hand once and then forgotten, so it starts here,
//! with the Engine, unless the person turned that off.
//!
//! What is remembered is a list of ids in [`FILE`], in the Engine's data
//! directory. A plugin is started only while it is installed, has a program,
//! and its manifest says how it starts ([`PluginStart`]) — the Engine still
//! knows none of the ids.
//!
//! First boot with no list at all: every installed plugin that says how it
//! starts is put on the list. That is the default a person expects from a
//! plugin that brought a background program; turning it off writes the list
//! without the id, and a written list is never re-seeded.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde_json::Value;

use super::{PluginError, PluginStore, Result};

/// The list of plugin ids that start with the Engine.
const FILE: &str = "plugins-autostart.json";
/// The format of that file.
const LIST_VERSION: u32 = 1;

/// The ids on the list, in the order they were written. A missing file reads as
/// an empty list; a damaged one does too — nothing is started by accident.
pub fn load(data_dir: &Path) -> BTreeSet<String> {
    let Ok(bytes) = fs::read(data_dir.join(FILE)) else {
        return BTreeSet::new();
    };
    let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
        return BTreeSet::new();
    };
    value
        .get("plugins")
        .and_then(Value::as_array)
        .map(|ids| {
            ids.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// True when the person has ever answered the question — even with "nobody".
pub fn stored(data_dir: &Path) -> bool {
    data_dir.join(FILE).is_file()
}

/// Write the list. An empty list is written too: it is the answer "none of
/// them", not the absence of an answer.
pub fn save(data_dir: &Path, ids: &BTreeSet<String>) -> Result<()> {
    fs::create_dir_all(data_dir)?;
    let body = serde_json::json!({
        "schema_version": LIST_VERSION,
        "plugins": ids.iter().collect::<Vec<_>>(),
    });
    crate::lock_file::write_owner_only_json(&data_dir.join(FILE), &body)
        .map_err(|error| PluginError::Settings(error.to_string()))
}

/// Put `id` on the list or take it off, and answer the list as it now stands.
/// Only a plugin that is installed and has a program can be added: a switch
/// that promises a start nobody can perform is worse than no switch.
pub fn set(store: &PluginStore, id: &str, enabled: bool) -> Result<Vec<String>> {
    let data_dir = store.data_dir().to_path_buf();
    let mut ids = load(&data_dir);
    if enabled {
        let installed = store
            .installed()
            .into_iter()
            .find(|plugin| plugin.id == id)
            .ok_or(PluginError::NotInstalled)?;
        if !installed.startable {
            return Err(PluginError::Manifest(format!(
                "у плагина «{id}» нет программы, которую можно запустить"
            )));
        }
        ids.insert(id.to_owned());
    } else {
        ids.remove(id);
    }
    save(&data_dir, &ids)?;
    Ok(ids.into_iter().collect())
}

/// Forget `id` when it is uninstalled or removed.
pub fn forget(store: &PluginStore, id: &str) -> Result<()> {
    let data_dir = store.data_dir().to_path_buf();
    if !data_dir.join(FILE).is_file() {
        return Ok(());
    }
    let mut ids = load(&data_dir);
    if ids.remove(id) {
        save(&data_dir, &ids)?;
    }
    Ok(())
}

/// What the Engine does at boot: make sure the list exists, then start what is
/// on it. Never fails the Engine — a plugin that will not start is logged and
/// skipped, the same way a missing program is.
pub async fn on_boot() {
    if !enabled_here() {
        return;
    }
    let store = match PluginStore::new() {
        Ok(store) => store,
        Err(error) => {
            log(&format!("plugin autostart: no store ({error})"));
            return;
        }
    };
    if !stored(store.data_dir()) {
        // First boot with this list: the catalog is what tells a plugin how it
        // starts, and the install may predate that field.
        if let Ok(answer) = super::catalog::fetch().await {
            let installed = store.installed();
            if let Err(error) = super::sync_manifests(&store, &answer.catalog, &installed) {
                log(&format!(
                    "plugin autostart: manifests not refreshed ({error})"
                ));
            }
        }
        let ids: BTreeSet<String> = store
            .installed()
            .into_iter()
            .filter(|plugin| plugin.startable)
            .map(|plugin| plugin.id)
            .collect();
        if !ids.is_empty() {
            if let Err(error) = save(store.data_dir(), &ids) {
                log(&format!("plugin autostart: list not written ({error})"));
            }
        }
    }
    let ids = load(store.data_dir());
    if ids.is_empty() {
        return;
    }
    let installed = store.installed();
    for plugin in installed.iter().filter(|plugin| ids.contains(&plugin.id)) {
        match start(&store, plugin) {
            Ok(true) => log(&format!("plugin autostart: {} started", plugin.id)),
            Ok(false) => {}
            Err(error) => log(&format!("plugin autostart: {} — {error}", plugin.id)),
        }
    }
}

/// Start one plugin the way its manifest says. `false` when there was nothing
/// to do (no program, already running, no start line).
fn start(store: &PluginStore, plugin: &super::InstalledPlugin) -> Result<bool> {
    if !plugin.startable || plugin.running {
        return Ok(false);
    }
    let Some(args) = store.start_args(&plugin.id) else {
        return Ok(false);
    };
    store.launch_with(&plugin.id, &args)?;
    Ok(true)
}

/// A boot in a test process must not start anybody's programs.
fn enabled_here() -> bool {
    !cfg!(test) && std::env::var_os("MUNDUS_TEST_MODE").is_none()
}

fn log(message: &str) {
    crate::observability::stderr(format!("[mundus-engine] {message}"));
}
