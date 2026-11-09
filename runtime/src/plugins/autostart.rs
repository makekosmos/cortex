//! Plugins that start with the Engine.
//!
//! The Engine already starts at sign-in (`<exe> --start`). A plugin whose
//! window is not a window — the dictation pill is one — has nothing to gain
//! from being launched by hand once and then forgotten, so it starts here,
//! with the Engine, unless the person turned that off.
//!
//! What is remembered is a list of [`Entry`]s in [`FILE`], in the Engine's data
//! directory: the plugin's id and the command line it starts with. The command
//! line is written when the person answers, taken from the manifest's
//! [`PluginStart`] — so a catalog that later says less (an older copy, an
//! offline cache) cannot silently stop a pill that was already promised to
//! start. The Engine still knows none of the ids.
//!
//! First boot with no list at all: every installed plugin whose manifest says
//! how it starts is put on the list. That is what a person expects from a
//! plugin that brought a background program; turning it off writes the list
//! without the id, and a written list is never re-seeded.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{PluginError, PluginStore, Result};

/// The list of plugins that start with the Engine.
const FILE: &str = "plugins-autostart.json";
/// The format of that file.
const LIST_VERSION: u32 = 1;

/// One plugin on the list: which, with what, and whether it still starts. A
/// switched-off entry keeps its command line, so turning it back on does not
/// depend on a manifest that may have forgotten the line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub id: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "yes")]
    pub enabled: bool,
}

fn yes() -> bool {
    true
}

/// The list as it stands. A missing file reads as an empty list; a damaged one
/// does too — nothing is started by accident.
pub fn load(data_dir: &Path) -> Vec<Entry> {
    let Ok(bytes) = fs::read(data_dir.join(FILE)) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
        return Vec::new();
    };
    let Some(plugins) = value.get("plugins").and_then(Value::as_array) else {
        return Vec::new();
    };
    plugins
        .iter()
        .filter_map(|entry| serde_json::from_value::<Entry>(entry.clone()).ok())
        .collect()
}

/// True when the person has ever answered the question — even with "nobody".
pub fn stored(data_dir: &Path) -> bool {
    data_dir.join(FILE).is_file()
}

/// Write the list. An empty list is written too: it is the answer "none of
/// them", not the absence of an answer.
pub fn save(data_dir: &Path, entries: &[Entry]) -> Result<()> {
    fs::create_dir_all(data_dir)?;
    let body = serde_json::json!({
        "schema_version": LIST_VERSION,
        "plugins": entries,
    });
    crate::lock_file::write_owner_only_json(&data_dir.join(FILE), &body)
        .map_err(|error| PluginError::Settings(error.to_string()))
}

/// Is `id` on the list, and starting?
pub fn starts_with_engine(data_dir: &Path, id: &str) -> bool {
    load(data_dir)
        .iter()
        .any(|entry| entry.id == id && entry.enabled)
}

/// Has this plugin ever been answered for (on or off)?
pub fn knows(data_dir: &Path, id: &str) -> bool {
    load(data_dir).iter().any(|entry| entry.id == id)
}

/// Put `id` on the list or take it off, and answer the ids as they now stand.
/// Only a plugin that is installed and says how it starts can be added: a
/// switch that promises a start nobody can perform is worse than no switch.
pub fn set(store: &PluginStore, id: &str, enabled: bool) -> Result<Vec<String>> {
    let data_dir = store.data_dir().to_path_buf();
    let mut entries = load(&data_dir);
    // The manifest beside the install may lag behind (an older catalog, an
    // offline cache): a command line already answered for stays, and a plugin
    // that never had one may only be switched on when its manifest says how.
    let known = entries
        .iter()
        .find(|entry| entry.id == id)
        .map(|entry| entry.args.clone());
    entries.retain(|entry| entry.id != id);
    if enabled {
        if !store.installed().iter().any(|plugin| plugin.id == id) {
            return Err(PluginError::NotInstalled);
        }
        let args = store.start_args(id).or(known).ok_or_else(|| {
            PluginError::Manifest(format!("у плагина «{id}» не сказано, как его запускать"))
        })?;
        entries.push(Entry {
            id: id.to_owned(),
            args,
            enabled: true,
        });
    } else if let Some(args) = known {
        entries.push(Entry {
            id: id.to_owned(),
            args,
            enabled: false,
        });
    }
    save(&data_dir, &entries)?;
    Ok(ids(&entries))
}

/// Forget `id` when it is uninstalled or removed.
pub fn forget(store: &PluginStore, id: &str) -> Result<()> {
    let data_dir = store.data_dir().to_path_buf();
    if !stored(&data_dir) {
        return Ok(());
    }
    let mut entries = load(&data_dir);
    let before = entries.len();
    entries.retain(|entry| entry.id != id);
    if entries.len() != before {
        save(&data_dir, &entries)?;
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
            log(&format!("no store ({error})"));
            return;
        }
    };
    if !stored(store.data_dir()) {
        seed(&store).await;
    }
    let entries = load(store.data_dir());
    if entries.is_empty() {
        log("list is empty: no plugin starts with the Engine");
        return;
    }
    let installed = store.installed();
    for entry in entries.iter().filter(|entry| entry.enabled) {
        let Some(plugin) = installed.iter().find(|plugin| plugin.id == entry.id) else {
            log(&format!("{}: not installed", entry.id));
            continue;
        };
        if plugin.running {
            log(&format!("{}: already running", entry.id));
            continue;
        }
        match store.launch_with(&entry.id, &entry.args) {
            Ok(()) => log(&format!("{}: started", entry.id)),
            Err(error) => log(&format!("{}: {error}", entry.id)),
        }
    }
}

/// The first answer: everything installed that says how it starts. The catalog
/// is asked once here because an install made before `start` existed has no
/// such line of its own.
async fn seed(store: &PluginStore) {
    if let Ok(answer) = super::catalog::fetch().await {
        let installed = store.installed();
        if let Err(error) = super::sync_manifests(store, &answer.catalog, &installed) {
            log(&format!("manifests not refreshed ({error})"));
        }
    }
    let entries: Vec<Entry> = store
        .installed()
        .into_iter()
        .filter(|plugin| plugin.startable)
        .map(|plugin| Entry {
            id: plugin.id,
            args: Vec::new(),
            enabled: true,
        })
        .collect();
    if entries.is_empty() {
        log("nothing to seed: no installed plugin says how it starts");
        return;
    }
    // The manifest, not the catalog, is what a seeded entry starts with.
    let entries: Vec<Entry> = entries
        .into_iter()
        .map(|entry| Entry {
            args: store.start_args(&entry.id).unwrap_or_default(),
            ..entry
        })
        .collect();
    if let Err(error) = save(store.data_dir(), &entries) {
        log(&format!("list not written ({error})"));
    }
}

fn ids(entries: &[Entry]) -> Vec<String> {
    entries
        .iter()
        .filter(|entry| entry.enabled)
        .map(|entry| entry.id.clone())
        .collect()
}

/// A boot in a test process must not start anybody's programs.
fn enabled_here() -> bool {
    !cfg!(test) && std::env::var_os("MUNDUS_TEST_MODE").is_none()
}

fn log(message: &str) {
    // The Engine log, not stderr: a plugin that did not start is something to
    // read in the morning, and this process is often started without a console.
    tracing::info!(target: "plugin_autostart", "{message}");
}
