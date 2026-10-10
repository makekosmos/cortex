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
                    let state = match result {
                        Ok(v) => {
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
