//! Active-page background updates keep snapshots visible (no Loading flash).
use super::*;

impl ManagerApp {
    pub(super) fn poll_views(&mut self) {
        if Instant::now() >= self.appearance.next_poll {
            self.appearance.next_poll = Instant::now() + std::time::Duration::from_secs(2);
            if !self.background_slots.contains("appearance")
                && !self.background_slots.contains("appearance.set")
                && !matches!(self.slots.get("appearance"), Some(Slot::Loading))
            {
                self.refresh("appearance", "appearance.get", json!({}));
            }
        }
        if self.view == View::About && Instant::now() >= self.next_about_poll {
            self.next_about_poll = Instant::now() + std::time::Duration::from_secs(5);
            if !matches!(self.slots.get("about.health"), Some(Slot::Loading)) {
                self.refresh_status("about.health", "health");
            }
        }
        if self.view == View::Sync && Instant::now() >= self.next_sync_poll {
            self.next_sync_poll = Instant::now() + std::time::Duration::from_secs(3);
            if !self.background_slots.contains("sync.snapshot")
                && !matches!(self.slots.get("sync.snapshot"), Some(Slot::Loading))
            {
                self.refresh("sync.snapshot", "get_sync_snapshot", json!({}));
            }
            if self.sync_pairing_open
                && self.data("sync.ticket").is_null()
                && !self.background_slots.contains("sync.ticket")
                && !matches!(self.slots.get("sync.ticket"), Some(Slot::Loading))
            {
                self.refresh("sync.ticket", "get_own_iroh_ticket", json!({}));
            }
        }
        // Model downloads run as Engine-side tasks; progress is only visible
        // inside the `list_local_models` snapshot. Poll every second while a
        // download runs, slower otherwise — a download can also be started
        // outside this Manager (another client), which only the poll sees.
        if self.view == View::Models && Instant::now() >= self.next_models_poll {
            let downloading = self
                .data("models.list")
                .get("models")
                .and_then(Value::as_array)
                .is_some_and(|models| {
                    models.iter().any(|model| {
                        model.get("downloading").and_then(Value::as_bool) == Some(true)
                    })
                });
            self.next_models_poll =
                Instant::now() + std::time::Duration::from_secs(if downloading { 1 } else { 5 });
            if !self.background_slots.contains("models.list") {
                self.refresh("models.list", "dictation.list_local_models", json!({}));
            }
        }
        // Native app installs run as Engine-side background jobs.
        if self.view == View::Packages && Instant::now() >= self.next_store_poll {
            self.next_store_poll = Instant::now() + std::time::Duration::from_secs(1);
            let installing = self
                .data("store.apps")
                .get("apps")
                .and_then(Value::as_array)
                .is_some_and(|apps| {
                    apps.iter()
                        .any(|a| a.get("state").and_then(Value::as_str) == Some("installing"))
                });
            if installing {
                self.refresh("store.apps", "apps.list", json!({}));
            }
        }
        // App-wide (KOS-355): the update overlay must see `updater.status`
        // from any page, not just About.
        if Instant::now() >= self.next_updater_poll {
            self.next_updater_poll = Instant::now() + std::time::Duration::from_secs(1);
            // Debug-only preview hook (KOS-355): force overlay states without
            // a real updater feed. Prefer control file for live switching:
            //   echo available >/tmp/mundus-debug-update-overlay
            //   echo 'downloading 42' >/tmp/mundus-debug-update-overlay
            // Env fallback: MUNDUS_DEBUG_UPDATE_OVERLAY=available|downloading|downloaded|error
            #[cfg(debug_assertions)]
            if let Some(status) = debug_update_overlay_status() {
                self.slots.insert("upd.mundus".into(), Slot::Ready(status));
            } else {
                self.send(Command::Rpc {
                    slot: "upd.mundus".into(),
                    op: "updater.status",
                    params: json!({}),
                });
            }
            #[cfg(not(debug_assertions))]
            self.send(Command::Rpc {
                slot: "upd.mundus".into(),
                op: "updater.status",
                params: json!({}),
            });
        }
    }
}

/// Debug-builds only (KOS-355): inject Engine-shaped `updater.status` for
/// live overlay previews. Compiled out of release builds entirely.
#[cfg(debug_assertions)]
fn debug_update_overlay_status() -> Option<serde_json::Value> {
    let raw = std::fs::read_to_string("/tmp/mundus-debug-update-overlay")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("MUNDUS_DEBUG_UPDATE_OVERLAY").ok())?;
    let mut parts = raw.split_whitespace();
    let state = parts.next()?.to_ascii_lowercase();
    if !matches!(
        state.as_str(),
        "available" | "downloading" | "downloaded" | "error"
    ) {
        return None;
    }
    let percent: f64 = parts
        .next()
        .and_then(|p| p.parse().ok())
        .or_else(|| {
            std::env::var("MUNDUS_DEBUG_UPDATE_PERCENT")
                .ok()
                .and_then(|p| p.parse().ok())
        })
        .unwrap_or(if state == "downloading" { 42.0 } else { 100.0 });
    Some(json!({
        "state": state,
        "currentVersion": "0.10.2",
        "newVersion": "0.10.3",
        "percent": percent,
        "canInstall": true,
        "message": "preview: forced by MUNDUS_DEBUG_UPDATE_OVERLAY",
    }))
}

#[cfg(test)]
mod tests {
    use super::{Instant, Slot, View};
    use gpui::TestAppContext;
    use serde_json::json;

    #[gpui::test]
    fn sync_poll_preserves_snapshot_and_skips_inflight_requests(cx: &mut TestAppContext) {
        let (manager, cx) = crate::a11y_tests::launch(cx);
        manager.update(cx, |app, _| {
            app.view = View::Sync;
            app.slots
                .insert("sync.snapshot".into(), Slot::Ready(json!({"peers":[]})));
            app.poll_views();
            assert!(matches!(
                app.slots.get("sync.snapshot"),
                Some(Slot::Ready(_))
            ));
            assert!(app.background_slots.contains("sync.snapshot"));
            let count = app.background_slots.len();
            app.next_sync_poll = Instant::now();
            app.poll_views();
            assert_eq!(app.background_slots.len(), count);
        });
    }
}
