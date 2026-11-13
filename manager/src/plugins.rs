//! The plugin rows the pages render, shaped from what the Engine answers.
//!
//! `data.rs` maps one route's calls to the state; the plugin keys are the ones
//! whose answers name things the Manager was not built around, so they are
//! shaped here and `data` only hands them over.

use serde_json::{json, Value};

use crate::data::{array, text};

/// The answer for a `plugins.*` key, or `None` when the key is not one.
pub(crate) fn shape(key: &str, data: &Value) -> Option<Value> {
    Some(match key {
        "plugins.catalog" => json!({
            "available": array(data, &["/catalog/plugins"])
                .iter()
                .map(plugin_row)
                .collect::<Vec<_>>(),
            // A catalog the Engine kept from last time still names what can be
            // installed; the page says so rather than looking fresh.
            "stale": data.get("stale").and_then(Value::as_bool).unwrap_or(false),
            "url": text(data, &["/url"]).unwrap_or_default(),
        }),
        "plugins.installed" => {
            Value::Array(array(data, &["/plugins"]).iter().map(plugin_row).collect())
        }
        "plugins.current" => json!({
            "id": text(data, &["/id"]).unwrap_or_default(),
            "manifest": data.get("manifest").cloned().unwrap_or(Value::Null),
            "values": data.get("values").cloned().unwrap_or_else(|| json!({})),
            // The answer replaces the placeholder `data::pending` wrote, so the
            // page can tell a slow Engine from one with no such plugin.
            "loading": false,
        }),
        _ => return None,
    })
}

/// A plugin, from either side of the catalog (`plugins.catalog` names what can
/// be installed, `plugins.list` what is): one shape, because the page compares
/// them by id. The manifest is where a plugin describes itself, so a record
/// without one still shows the id — never a made-up name.
fn plugin_row(plugin: &Value) -> Value {
    let manifest = plugin.get("manifest").filter(|value| !value.is_null());
    // A record keeps the manifest beside it; a catalog row *is* one.
    let said = |path: &str| {
        manifest
            .and_then(|manifest| manifest.pointer(path))
            .or_else(|| plugin.pointer(path))
            .and_then(Value::as_str)
            .map(str::to_owned)
    };
    let id = text(plugin, &["/id"])
        .or_else(|| said("/id"))
        .unwrap_or_default();
    json!({
        "id": id.clone(),
        "name": said("/name").unwrap_or_else(|| id.clone()),
        "summary": said("/summary").unwrap_or_default(),
        "version": text(plugin, &["/version"]).unwrap_or_default(),
        "running": plugin.get("running").and_then(Value::as_bool).unwrap_or(false),
        // Whether the plugin brought a program of its own (a `builtin` does not).
        "executable": plugin.get("executable").is_some_and(Value::is_string),
        "settings": plugin.get("settings").cloned().unwrap_or(Value::Null),
    })
}
