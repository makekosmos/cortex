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
        "settings.get" => settings_get(&params),
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

async fn list() -> Result<Value> {
    let store = PluginStore::new()?;
    let installed = store.installed();
    Ok(serde_json::json!({ "plugins": installed }))
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

fn launch(params: &Value) -> Result<Value> {
    let store = PluginStore::new()?;
    let id = param_id(params)?;
    store.launch(&id)?;
    Ok(serde_json::json!({ "launched": id }))
}

fn settings_get(params: &Value) -> Result<Value> {
    let store = PluginStore::new()?;
    let id = param_id(params)?;
    let manifest = store.manifest(&id).ok_or(PluginError::NotInstalled)?;
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
    let values = store.set_settings(&manifest, values)?;
    Ok(serde_json::json!({ "id": id, "values": values }))
}

fn param_id(params: &Value) -> Result<String> {
    params
        .get("id")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| PluginError::Manifest("не указан id плагина".into()))
}
