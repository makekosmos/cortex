//! Active-page background updates keep snapshots visible (no Loading flash).
use super::*;

impl ManagerApp {
    pub(super) fn poll_views(&mut self) {
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
        if self.view == View::Updates && Instant::now() >= self.next_updater_poll {
            self.next_updater_poll = Instant::now() + std::time::Duration::from_secs(1);
            if self
                .worker
                .commands
                .send(Command::Rpc {
                    slot: "upd.mundus".into(),
                    op: "updater.status",
                    params: json!({}),
                })
                .is_err()
            {
                self.worker_dead = true;
            }
        }
    }
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
