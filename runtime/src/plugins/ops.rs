//! The operations the pages ask for, as `plugins.<op>` over the Engine socket.

use serde_json::Value;

use super::catalog;
use super::manifest::PluginManifest;
use super::{PluginError, PluginSource, PluginStore, Result};

/// `plugins.<subop>` — every operation the pages ask for, in one place.
pub async fn handle_plugin_op(subop: &str, params: Value) -> crate::plugins::OpResponse {
    let result = match subop {
        "list" => list().await,
        "catalog" => catalog::fetch().await.and_then(|answer| {
            serde_json::to_value(answer).map_err(|error| PluginError::Catalog(error.to_string()))
        }),
        "install" => install(&params).await,
        "remove" => remove(&params),
        "launch" => launch(&params),
        "autostart.set" => autostart_set(&params),
        "settings.get" => settings_get(&params).await,
        "settings.set" => settings_set(&params),
        other => Err(PluginError::Manifest(format!(
            "plugins.{other}: неизвестная операция"
        ))),
    };
    OpResponse::from(result)
}

/// One answer for the pages: `Ok` goes into `data`, `Err` into `error`.
pub struct OpResponse {
    pub ok: bool,
    pub data: Value,
    pub error: Option<String>,
}

impl OpResponse {
    pub fn ok(data: Value) -> Self {
        Self {
            ok: true,
            data,
            error: None,
        }
    }

    pub fn err(error: impl std::fmt::Display) -> Self {
        Self {
            ok: false,
            data: Value::Null,
            error: Some(error.to_string()),
        }
    }
}

impl From<Result<Value>> for OpResponse {
    fn from(result: Result<Value>) -> Self {
        match result {
            Ok(data) => Self::ok(data),
            Err(error) => Self::err(error),
        }
    }
}

/// Installed plugins. A plugin installed before manifests were kept (the
/// dictation app this Engine already ships) has no `plugin.json` of its own:
/// the catalog's copy is stored beside it, once, so the pages can name it and
/// open its settings — a plugin that is on disk is not "unknown" because it
/// arrived before the catalog did.
///
/// An install nothing describes is not a plugin: the same catalog installs
/// native apps, and a page that offered them a settings screen it cannot draw
/// would be worse than a page that leaves them out.
async fn list() -> Result<Value> {
    let store = PluginStore::new()?;
    let mut installed = store.installed();
    // The catalog describes the plugins the Engine has, and it moves: an
    // install adopted before the catalog named a field is brought up to date.
    if let Ok(answer) = catalog::fetch().await {
        if installed.iter().any(|plugin| plugin.manifest.is_none()) {
            for plugin in installed.iter_mut().filter(|p| p.manifest.is_none()) {
                if let Some(manifest) = adopt(&store, &answer.catalog, &plugin.id) {
                    plugin.settings = store.settings(&manifest).unwrap_or_default();
                    plugin.manifest = Some(manifest);
                }
            }
        }
        super::sync_manifests(&store, &answer.catalog, &installed)?;
        installed = store.installed();
    }
    installed.retain(|plugin| plugin.manifest.is_some());
    Ok(serde_json::json!({ "plugins": installed }))
}

/// The catalog's manifest for `id`, stored beside the install when it has none.
pub(super) fn adopt(
    store: &PluginStore,
    catalog: &catalog::Catalog,
    id: &str,
) -> Option<PluginManifest> {
    let manifest = catalog
        .plugins
        .iter()
        .find(|plugin| plugin.id == id)?
        .clone();
    store.save_manifest(&manifest).ok()?;
    Some(manifest)
}

async fn install(params: &Value) -> Result<Value> {
    let store = PluginStore::new()?;
    let manifest = resolve_manifest(&store, params).await?;
    if matches!(manifest.source, PluginSource::Builtin) {
        // Nothing to download: the manager already has it. Keep the manifest
        // so the pages can show it, and report the plugin as present.
        store.save_manifest(&manifest)?;
        return Ok(serde_json::json!({ "plugin": manifest, "installed": false }));
    }
    let installed = store.install(&manifest).await?;
    Ok(serde_json::json!({ "plugin": installed.manifest, "installed": true, "record": installed }))
}

/// A manifest from the catalog by id, or one passed inline (the Manager may
/// point at a manifest URL of its own).
async fn resolve_manifest(store: &PluginStore, params: &Value) -> Result<PluginManifest> {
    if let Some(inline) = params.get("manifest") {
        let manifest: PluginManifest = serde_json::from_value(inline.clone())
            .map_err(|error| PluginError::Manifest(error.to_string()))?;
        manifest.validate()?;
        return Ok(manifest);
    }
    let id = params
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| PluginError::Manifest("не указан id плагина".into()))?;
    let catalog = catalog::fetch().await?;
    let manifest = catalog
        .catalog
        .plugins
        .into_iter()
        .find(|plugin| plugin.id == id)
        .ok_or(PluginError::Unknown)?;
    if store.record(&manifest.id)?.is_some() && store.manifest(&manifest.id).is_none() {
        // Installed before manifests were kept: the catalog's copy describes it.
        store.save_manifest(&manifest)?;
    }
    Ok(manifest)
}

fn remove(params: &Value) -> Result<Value> {
    let store = PluginStore::new()?;
    let id = param_id(params)?;
    store.remove(&id)?;
    Ok(serde_json::json!({ "removed": id }))
}

/// `plugins.autostart.set { id, enabled }` — start this plugin with the
/// Engine, or stop doing that. Answers the list as it now stands.
fn autostart_set(params: &Value) -> Result<Value> {
    let store = PluginStore::new()?;
    let id = param_id(params)?;
    let enabled = params
        .get("enabled")
        .and_then(Value::as_bool)
        .ok_or_else(|| PluginError::Manifest("не сказано, включать ли автозапуск".into()))?;
    let ids = super::autostart::set(&store, &id, enabled)?;
    Ok(serde_json::json!({ "id": id, "autostart": enabled, "plugins": ids }))
}

fn launch(params: &Value) -> Result<Value> {
    let store = PluginStore::new()?;
    let id = param_id(params)?;
    store.launch(&id)?;
    Ok(serde_json::json!({ "launched": id }))
}

async fn settings_get(params: &Value) -> Result<Value> {
    let store = PluginStore::new()?;
    let id = param_id(params)?;
    let manifest = match store.manifest(&id) {
        Some(manifest) => manifest,
        // An install from before the catalog: ask the catalog who it is.
        None => {
            store.record(&id)?.ok_or(PluginError::NotInstalled)?;
            let answer = catalog::fetch().await?;
            adopt(&store, &answer.catalog, &id).ok_or(PluginError::Unknown)?
        }
    };
    let values = store.settings(&manifest)?;
    Ok(serde_json::json!({ "id": id, "manifest": manifest, "values": values }))
}

fn settings_set(params: &Value) -> Result<Value> {
    let store = PluginStore::new()?;
    let id = param_id(params)?;
    let manifest = store.manifest(&id).ok_or(PluginError::NotInstalled)?;
    let values = params
        .get("values")
        .and_then(Value::as_object)
        .ok_or_else(|| PluginError::Settings("нет значений".into()))?;
    let values = manifest.check_settings(values)?;
    let applies = manifest.applies(&values);
    let values = store.set_settings(&manifest, &values)?;
    let applies: Vec<Value> = applies
        .into_iter()
        .map(|apply| serde_json::json!({ "op": apply.op, "params": apply.params }))
        .collect();
    Ok(serde_json::json!({ "id": id, "values": values, "applies": applies }))
}

fn param_id(params: &Value) -> Result<String> {
    params
        .get("id")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| PluginError::Manifest("не указан id плагина".into()))
}
