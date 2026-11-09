//! What a person adds to Mundus, described by the plugin itself.
//!
//! A plugin is a manifest — `plugin.json` — and, if it has code of its own, a
//! program installed from one of its repository's GitHub releases. The
//! artifacts are the ones the shipped apps already use: `<id>/<version>/` under
//! the product-local `Apps` root, `install.json` as the current pointer, and an
//! archive verified against the release's `SHA256SUMS.txt` (`native_apps`).
//!
//! The Engine knows none of the ids. A new plugin is a new manifest in the
//! catalog ([`catalog`]), and every operation below is the same for all of them:
//!
//! | Operation                       | What it does                                  |
//! | ------------------------------- | --------------------------------------------- |
//! | `plugins.list`                  | installed plugins, their manifest and settings |
//! | `plugins.catalog`               | the catalog, cached; stale beats empty         |
//! | `plugins.install { id }`        | latest release → verified install → manifest   |
//! | `plugins.remove { id }`         | the install; user settings stay                |
//! | `plugins.launch { id }`         | the program, told where its settings are       |
//! | `plugins.autostart.set { id, enabled }` | start it with the Engine, or not  |
//! | `plugins.settings.get { id }`   | values merged over the manifest's defaults     |
//! | `plugins.settings.set { id, values }` | validated against the manifest, then written |
//!
//! Settings belong to the plugin: the manifest declares the fields
//! ([`SettingField`]) and where they are kept (`settings_file`, default
//! `<id>/config.json`), and the Engine only validates and stores them. A plugin
//! that already has a config file of its own keeps it — `dictation-config.json`
//! is one, and it reads the same bytes it always did.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::native_apps::{self, NativeAppInstall, NativeAppStore, NativeInstallSpec, MAX_ARCHIVE};

pub mod autostart;
pub mod catalog;
pub mod manifest;
pub mod ops;
pub mod release;
#[cfg(test)]
mod tests;

pub use manifest::{PluginManifest, PluginSource, PluginStart, SettingField, SettingKind};
pub use ops::{handle_plugin_op, OpResponse};

/// The manifest file inside a plugin's directory.
const PLUGIN_FILE: &str = "plugin.json";
/// Where settings live when the manifest does not say otherwise.
const DEFAULT_CONFIG: &str = "config.json";
/// Told to a launched plugin: which plugin it is, and where its settings are.
pub const ENV_PLUGIN_ID: &str = "MUNDUS_PLUGIN_ID";
pub const ENV_PLUGIN_CONFIG: &str = "MUNDUS_PLUGIN_CONFIG";

/// One failed plugin operation. The pages show these strings as they are, so
/// each one says what a person can act on.
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("не удалось прочитать данные: {0}")]
    Io(#[from] io::Error),
    #[error("описание плагина некорректно: {0}")]
    Manifest(String),
    #[error("нет такого плагина в каталоге")]
    Unknown,
    #[error("плагин не установлен")]
    NotInstalled,
    #[error("плагин запущен — сначала закройте его")]
    Running,
    #[error("для этой платформы плагин не публикуется")]
    NoRelease,
    #[error("каталог недоступен: {0}")]
    Catalog(String),
    #[error("установка не удалась: {0}")]
    Install(String),
    #[error("настройки не приняты: {0}")]
    Settings(String),
}

pub type Result<T> = std::result::Result<T, PluginError>;

/// A plugin that is on disk, with what the pages need to show it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InstalledPlugin {
    pub id: String,
    /// The manifest stored with the install; `None` for a plugin installed
    /// before manifests were kept (the catalog fills it in).
    pub manifest: Option<PluginManifest>,
    pub version: String,
    /// Absolute path of the program, when the plugin has one.
    pub executable: Option<String>,
    pub installed_at: u64,
    pub running: bool,
    /// Whether this plugin starts with the Engine (`plugins-autostart.json`).
    pub autostart: bool,
    /// Whether its manifest says how it starts at all — a plugin only launched
    /// by hand leaves the autostart switch out.
    pub startable: bool,
    /// Settings merged over the manifest's defaults.
    pub settings: Map<String, Value>,
}

/// Keep the manifests kept beside installs in step with the catalog. The
/// catalog is the plugin's own description and it moves — a new setting, the
/// command line it starts with — while the copy beside the install is there so
/// the pages work without the catalog at all. Nothing a person owns lives in a
/// manifest (settings are a file of their own), so the catalog's copy wins.
pub fn sync_manifests(
    store: &PluginStore,
    catalog: &catalog::Catalog,
    installed: &[InstalledPlugin],
) -> Result<usize> {
    let mut updated = 0;
    for plugin in installed {
        let Some(described) = catalog.plugins.iter().find(|entry| entry.id == plugin.id) else {
            continue;
        };
        if plugin.manifest.as_ref() == Some(described) {
            continue;
        }
        store.save_manifest(described)?;
        updated += 1;
    }
    Ok(updated)
}

/// The installed plugins, at one root: the product-local `Apps` directory.
pub struct PluginStore {
    apps: NativeAppStore,
    data_dir: PathBuf,
}

impl PluginStore {
    /// The store the Engine uses: `%LOCALAPPDATA%\Mundus\Apps` and the Engine's
    /// data directory (for plugins whose settings file lives beside it).
    pub fn new() -> Result<Self> {
        Self::with_data_dir(
            crate::data_dir::mundus_data_dir()
                .map_err(|error| PluginError::Manifest(format!("нет каталога данных: {error}")))?,
        )
    }

    pub fn with_data_dir(data_dir: PathBuf) -> Result<Self> {
        let root = native_apps::native_apps_root()
            .map_err(|error| PluginError::Install(error.to_string()))?;
        Self::at(root, data_dir)
    }

    pub fn at(root: PathBuf, data_dir: PathBuf) -> Result<Self> {
        let apps =
            NativeAppStore::new(root).map_err(|error| PluginError::Install(error.to_string()))?;
        Ok(Self { apps, data_dir })
    }

    pub fn apps_root(&self) -> &Path {
        self.apps.root()
    }

    /// Every `<id>` with an `install.json`, with its manifest and settings.
    pub fn installed(&self) -> Vec<InstalledPlugin> {
        let list = autostart::load(&self.data_dir);
        self.apps
            .list()
            .into_iter()
            .map(|record| {
                let starts = list.iter().any(|entry| entry.id == record.id);
                self.describe(&record, starts)
            })
            .collect()
    }

    /// The Engine's data directory: where a plugin's settings file and the
    /// autostart list live.
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn record(&self, id: &str) -> Result<Option<NativeAppInstall>> {
        self.apps
            .current(id)
            .map_err(|error| PluginError::Install(error.to_string()))
    }

    /// The manifest kept with the install, if the install wrote one.
    pub fn manifest(&self, id: &str) -> Option<PluginManifest> {
        let path = self.apps.root().join(id).join(PLUGIN_FILE);
        let bytes = fs::read(path).ok()?;
        let manifest: PluginManifest = serde_json::from_slice(&bytes).ok()?;
        manifest.validate().ok()?;
        (manifest.id == id).then_some(manifest)
    }

    fn describe(&self, record: &NativeAppInstall, autostart: bool) -> InstalledPlugin {
        let manifest = self.manifest(&record.id);
        // Offered when the Engine can start it as described — the manifest says
        // how, or an earlier answer already wrote the command line.
        let startable = manifest.as_ref().is_some_and(|m| m.start.is_some()) || autostart;
        let settings = manifest
            .as_ref()
            .and_then(|manifest| self.settings(manifest).ok())
            .unwrap_or_default();
        InstalledPlugin {
            id: record.id.clone(),
            manifest,
            version: record.version.clone(),
            executable: self
                .apps
                .executable_path(&record.id)
                .map(|path| path.to_string_lossy().into_owned()),
            installed_at: record.installed_at,
            running: self.apps.app_is_running(&record.id).unwrap_or(false),
            autostart,
            startable,
            settings,
        }
    }

    /// Install `manifest` from its latest release: fetch the sums, download the
    /// asset, verify it, extract it, flip the pointer, then keep the manifest
    /// beside the install so the pages do not need the catalog to describe it.
    pub async fn install(&self, manifest: &PluginManifest) -> Result<InstalledPlugin> {
        manifest.validate()?;
        let release = manifest
            .source
            .target()
            .ok_or_else(|| PluginError::Manifest("у плагина нет файла для установки".into()))?;
        let target = native_apps::host_app_target().ok_or(PluginError::NoRelease)?;
        let probe = native_apps::releases::ReleaseProbe::new()
            .map_err(|error| PluginError::Install(error.to_string()))?;
        let info = match native_apps::releases::fetch_latest(&probe, release, target, None)
            .await
            .map_err(|error| PluginError::Install(error.to_string()))?
        {
            native_apps::releases::ReleaseCheck::Fresh { info, .. } => info,
            native_apps::releases::ReleaseCheck::NotModified => {
                return Err(PluginError::Install("нет данных о релизе".into()));
            }
        };
        let url = native_apps::releases::asset_url(
            &probe.base,
            release.repository,
            &info.tag,
            &info.asset,
        );
        let staging = self
            .apps
            .root()
            .join(native_apps::unique_temp_name(".plugin-download-"));
        let download = native_apps::releases::TempDownload::at(staging);
        let (sha256, size) =
            native_apps::releases::download(&probe, &url, download.path(), &mut |_, _| {})
                .await
                .map_err(|error| PluginError::Install(error.to_string()))?;
        if sha256 != info.sha256 {
            return Err(PluginError::Install("файл не совпал с SHA256SUMS".into()));
        }
        if size > MAX_ARCHIVE {
            return Err(PluginError::Install("файл больше допустимого".into()));
        }
        let spec = NativeInstallSpec {
            id: manifest.id.clone(),
            version: info.version.clone(),
            executable: release.executable(target),
            sha256: info.sha256.clone(),
            size,
            repository: release.repository.to_owned(),
            release_tag: info.tag.clone(),
        };
        self.apps
            .install_archive(&spec, download.path())
            .map_err(|error| PluginError::Install(error.to_string()))?;
        self.save_manifest(manifest)?;
        let record = self
            .record(&manifest.id)?
            .ok_or_else(|| PluginError::Install("установка не записалась".into()))?;
        // A plugin that brought a background program starts with the Engine
        // from now on: that is what it was installed for.
        if manifest.start.is_some() {
            if let Err(error) = autostart::set(self, &manifest.id, true) {
                crate::observability::stderr(format!(
                    "[mundus-engine] plugin {}: not put on the autostart list ({error})",
                    manifest.id
                ));
            }
        }
        Ok(self.describe(
            &record,
            autostart::starts_with_engine(&self.data_dir, &manifest.id),
        ))
    }

    pub fn save_manifest(&self, manifest: &PluginManifest) -> Result<()> {
        manifest.validate()?;
        let dir = self.apps.root().join(&manifest.id);
        fs::create_dir_all(&dir)?;
        let bytes = serde_json::to_vec_pretty(manifest)
            .map_err(|error| PluginError::Manifest(error.to_string()))?;
        let temp = dir.join(native_apps::unique_temp_name(".plugin-"));
        fs::write(&temp, &bytes)?;
        match fs::rename(&temp, dir.join(PLUGIN_FILE)) {
            Ok(()) => Ok(()),
            Err(error) => {
                let _ = fs::remove_file(&temp);
                Err(error.into())
            }
        }
    }

    /// Remove the install. Settings are the person's, not the archive's, so
    /// they stay where they are — reinstalling picks them up again.
    pub fn remove(&self, id: &str) -> Result<()> {
        let removed = self.apps.uninstall(id).map_err(|error| match error {
            native_apps::NativeAppError::Running => PluginError::Running,
            other => PluginError::Install(other.to_string()),
        });
        if removed.is_ok() {
            autostart::forget(self, id)?;
        }
        removed
    }

    pub fn executable(&self, id: &str) -> Option<PathBuf> {
        self.apps.executable_path(id)
    }

    /// Start the plugin's program, telling it which plugin it is and where its
    /// settings are. A plugin without a program (`builtin`) has nothing to do.
    pub fn launch(&self, id: &str) -> Result<()> {
        self.launch_with(id, &[])
    }

    /// How this plugin starts when it starts with the Engine: the manifest's
    /// `start.args`, or `None` when it says nothing (a plugin launched by hand).
    pub fn start_args(&self, id: &str) -> Option<Vec<String>> {
        self.manifest(id)?.start.map(|start| start.args)
    }

    /// Start the plugin's program with `args`.
    pub fn launch_with(&self, id: &str, args: &[String]) -> Result<()> {
        let Some(executable) = self.executable(id) else {
            return Err(PluginError::NotInstalled);
        };
        let config = match self.manifest(id) {
            Some(manifest) => self.config_path(&manifest)?,
            None => self.apps.root().join(id).join(DEFAULT_CONFIG),
        };
        if let Some(parent) = config.parent() {
            fs::create_dir_all(parent)?;
        }
        let args: Vec<&std::ffi::OsStr> = args.iter().map(|arg| arg.as_ref()).collect();
        native_apps::spawn_detached_with(
            &executable,
            &args,
            &[
                (ENV_PLUGIN_ID, std::ffi::OsStr::new(id)),
                (ENV_PLUGIN_CONFIG, config.as_os_str()),
            ],
        )
        .map_err(PluginError::Io)
    }

    /// Where this plugin's settings are written: its own file inside the
    /// plugin directory, or the one the manifest names in the Engine's data
    /// directory (a plugin that had a config file before it was a plugin).
    pub fn config_path(&self, manifest: &PluginManifest) -> Result<PathBuf> {
        match manifest.settings_file.as_deref() {
            Some(name) => {
                if !manifest::valid_file_name(name) {
                    return Err(PluginError::Manifest(format!(
                        "settings_file «{name}» — не имя файла"
                    )));
                }
                Ok(self.data_dir.join(name))
            }
            None => Ok(self.apps.root().join(&manifest.id).join(DEFAULT_CONFIG)),
        }
    }

    /// The settings as stored, merged over the manifest's defaults.
    pub fn settings(&self, manifest: &PluginManifest) -> Result<Map<String, Value>> {
        let stored = self.stored_settings(manifest)?;
        let mut merged = Map::new();
        for field in &manifest.settings {
            let value = stored
                .get(&field.key)
                .cloned()
                .unwrap_or_else(|| field.value.clone());
            merged.insert(field.key.clone(), value);
        }
        Ok(merged)
    }

    /// Validate `values` against the manifest, then merge them into the file —
    /// keys the manifest does not declare are refused, and keys the file has
    /// that the manifest does not (yet) are kept, so two views of one file do
    /// not erase each other.
    pub fn set_settings(
        &self,
        manifest: &PluginManifest,
        values: &Map<String, Value>,
    ) -> Result<Map<String, Value>> {
        let checked = manifest.check_settings(values)?;
        let path = self.config_path(manifest)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut stored = self.stored_settings(manifest)?;
        for (key, value) in checked {
            stored.insert(key, value);
        }
        let json = Value::Object(stored.clone());
        crate::lock_file::write_owner_only_json(&path, &json)
            .map_err(|error| PluginError::Settings(error.to_string()))?;
        self.settings(manifest)
    }

    fn stored_settings(&self, manifest: &PluginManifest) -> Result<Map<String, Value>> {
        let path = self.config_path(manifest)?;
        match fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice::<Value>(&bytes) {
                Ok(Value::Object(map)) => Ok(map),
                _ => Err(PluginError::Settings(format!(
                    "{} — не объект JSON",
                    path.display()
                ))),
            },
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Map::new()),
            Err(error) => Err(PluginError::Io(error)),
        }
    }
}
