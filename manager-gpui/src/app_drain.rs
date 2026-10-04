//! Apply Engine worker replies to the Manager's controlled UI state.
use super::*;

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
            if reply.slot == "appearance.set" {
                self.background_slots.remove("appearance.set");
                match reply.result {
                    Ok(value) => {
                        if self.appearance.ingest(&value) {
                            self.slots.insert("appearance".into(), Slot::Ready(value));
                        }
                    }
                    Err(error) => self.error = Some(error),
                }
            } else if reply.slot == "@action" {
                self.action_busy = false;
                match reply.result {
                    Ok(_) => {
                        self.notice = Some("Выполнено.".into());
                        self.invalidated_slots.extend(self.slots.keys().cloned());
                        views::load(self.view, self);
                    }
                    Err(e) => self.error = Some(e),
                }
            } else if reply.slot == "pkg.open" {
                self.open_reply(reply.result);
            } else if reply.slot == "conn.login" {
                self.login_reply(reply.result);
            } else if reply.slot == "apps.op" {
                // Pull fresh app rows after background install/update starts.
                match reply.result {
                    Ok(_) => self.refresh("store.apps", "apps.list", json!({})),
                    Err(e) => self.error = Some(e),
                }
            } else if reply.slot == "disclosure" {
                match reply.result {
                    Ok(v) => self.disclosure = Some(v),
                    Err(e) => self.error = Some(e),
                }
            } else if reply.slot == "store.ext" {
                self.external_url_reply(reply.result);
            } else {
                let slot = reply.slot.clone();
                let background = self.background_slots.remove(&slot);
                self.slots.insert(
                    slot.clone(),
                    match reply.result {
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
                    },
                );
                if slot == "usage.report" {
                    self.rebuild_usage_rows();
                }
            }
            cx.notify();
        }
        self.poll_views();
    }
}
