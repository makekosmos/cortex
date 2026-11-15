//! The plugin that draws the pill.
//!
//! The Engine owns the dot scope and broadcasts every step; the picture itself
//! is a plugin process, so a change to the pill ships as a plugin update and
//! never as a new Engine. Nothing here draws — this only makes sure the process
//! that does is up.
//!
//! It is kept warm on purpose: a pill that boots on the first frame appears a
//! second late, after the person has already started talking. Started when the
//! Engine starts, before the first dictation of the session.

use super::PluginStore;

/// Start the installed plugin whose manifest asks for a `pill` overlay, unless
/// it is already running.
///
/// Best effort, like the tray: a plugin that cannot start is a pill that does
/// not appear, never a dictation that does not work.
pub fn ensure_running() {
    let store = match PluginStore::new() {
        Ok(store) => store,
        Err(error) => {
            tracing::warn!(%error, "pill: plugin store unavailable");
            return;
        }
    };
    let Some(plugin) = store.installed().into_iter().find(|plugin| {
        plugin.executable.is_some()
            && plugin
                .manifest
                .as_ref()
                .and_then(|manifest| manifest.overlay.as_ref())
                .is_some_and(|overlay| overlay.kind == "pill")
    }) else {
        return;
    };
    if plugin.running {
        return;
    }
    match store.launch(&plugin.id) {
        Ok(()) => tracing::info!(id = %plugin.id, "pill process started"),
        Err(error) => tracing::warn!(id = %plugin.id, %error, "pill process did not start"),
    }
}
