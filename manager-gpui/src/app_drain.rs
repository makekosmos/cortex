//! Apply Engine worker replies to the Manager's controlled UI state.
use super::*;
use crate::worker::Reply;

impl ManagerApp {
    pub(super) fn drain(&mut self, cx: &mut Context<Self>) {
        loop {
            let reply = match self.worker.replies.try_recv() {
                Ok(reply) => reply,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.worker_dead = true;
                    self.error =
                        Some("Соединение с Engine завершено. Перезапустите приложение.".into());
                    cx.notify();
                    break;
                }
            };
            let Reply { slot, result } = reply;
            match slot.as_str() {
                "appearance.set" => {
                    self.background_slots.remove("appearance.set");
                    match result {
                        Ok(value) => {
                            if self.appearance.ingest(&value) {
                                self.slots.insert("appearance".into(), Slot::Ready(value));
                            }
                        }
                        Err(error) => self.error = Some(error),
                    }
                }
                "@action" => self.action_reply(result),
                "pkg.open" => self.open_reply(result),
                "conn.login" => self.login_reply(result),
                "store.ext" => self.external_url_reply(result),
                // Pull fresh app rows after background install/update starts.
                "apps.op" => match result {
                    Ok(_) => self.refresh("store.apps", "apps.list", json!({})),
                    Err(e) => self.error = Some(e),
                },
                "disclosure" => match result {
                    Ok(v) => self.disclosure = Some(v),
                    Err(e) => self.error = Some(e),
                },
                _ => {
                    let background = self.background_slots.remove(&slot);
                    let previous_error = self.error.clone();
                    let state = match result {
                        Ok(mut v) => {
                            if slot == "data.storage" {
                                crate::views::storage::normalize(&mut v);
                            }
                            self.invalidated_slots.remove(&slot);
                            if !background && self.error.is_some() {
                                self.error = None;
                            }
                            Slot::Ready(v)
                        }
                        Err(e) => {
                            self.error = Some(e.clone());
                            Slot::Failed(e)
                        }
                    };
                    let unchanged = match (self.slots.get(&slot), &state) {
                        (Some(Slot::Ready(before)), Slot::Ready(after)) => before == after,
                        (Some(Slot::Failed(before)), Slot::Failed(after)) => before == after,
                        _ => false,
                    };
                    if unchanged && previous_error == self.error {
                        continue;
                    }
                    self.slots.insert(slot.clone(), state);
                    if slot == "usage.report" {
                        self.rebuild_usage_rows();
                    }
                }
            }
            cx.notify();
        }
        self.poll_views();
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        app::Slot,
        worker::{Reply, Worker},
    };
    use gpui::TestAppContext;
    use serde_json::json;
    use std::{cell::Cell, rc::Rc, sync::mpsc};

    #[gpui::test]
    fn identical_storage_reply_does_not_notify_but_changed_size_does(cx: &mut TestAppContext) {
        let (manager, cx) = crate::a11y_tests::launch(cx);
        let (commands, _requests) = mpsc::channel();
        let (results, replies) = mpsc::channel();
        let mut initial = json!({"categories":[{"id":"database", "bytes":4096}]});
        crate::views::storage::normalize(&mut initial);
        manager.update(cx, |app, _| {
            app.worker = Worker { commands, replies };
            app.slots
                .insert("data.storage".into(), Slot::Ready(initial.clone()));
        });
        let notifications = Rc::new(Cell::new(0));
        let observed = notifications.clone();
        let _subscription =
            cx.update(|_, cx| cx.observe(&manager, move |_, _| observed.set(observed.get() + 1)));
        results
            .send(Reply {
                slot: "data.storage".into(),
                result: Ok(initial.clone()),
            })
            .unwrap();
        manager.update(cx, |app, cx| app.drain(cx));
        assert_eq!(notifications.get(), 0);
        initial["categories"][0]["bytes"] = json!(8192);
        results
            .send(Reply {
                slot: "data.storage".into(),
                result: Ok(initial),
            })
            .unwrap();
        manager.update(cx, |app, cx| app.drain(cx));
        assert_eq!(notifications.get(), 1);
    }
}
