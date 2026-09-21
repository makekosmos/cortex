//! Root entity: sidebar nav, per-view data slots, Engine error banner,
//! confirmation modal. Every Engine answer lands in `slots` keyed by a
//! string the issuing view chose — views render whatever arrived.
use std::collections::HashMap;
use std::sync::mpsc::TryRecvError;

use ::gpui::{prelude::*, *};
use gpui_component::input::InputState;
use serde_json::Value;

use crate::views::{self, StoreTab, View};
use crate::worker::{Command, Worker};

pub enum Slot {
    Loading,
    Ready(Value),
    Failed(String),
}

pub struct Confirm {
    pub title: String,
    pub body: String,
    pub op: &'static str,
    pub params: Value,
}

pub struct ManagerApp {
    pub view: View,
    worker: Worker,
    pub slots: HashMap<String, Slot>,
    pub inputs: HashMap<String, Entity<InputState>>,
    /// Transient success line; cleared on the next action.
    pub notice: Option<String>,
    /// Persistent Engine error (banner until a retry succeeds).
    pub error: Option<String>,
    /// Object type selected inside Данные.
    pub data_type: Option<String>,
    pub store_tab: StoreTab,
    pub confirm: Option<Confirm>,
    /// packages.disclosure payload waiting for install consent.
    pub disclosure: Option<Value>,
    /// package_id the disclosure was fetched for — install proceeds on consent.
    pub pending_install: Option<String>,
    /// Store listing shown in the detail overlay.
    pub detail: Option<Value>,
    pub(crate) action_busy: bool,
    worker_dead: bool,
}

impl ManagerApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let data_dir = crate::engine::data_dir().ok();
        let mut this = Self {
            view: View::Data,
            worker: Worker::start(data_dir),
            slots: HashMap::new(),
            inputs: HashMap::new(),
            notice: None,
            error: None,
            data_type: None,
            store_tab: StoreTab::Catalog,
            confirm: None,
            disclosure: None,
            pending_install: None,
            detail: None,
            action_busy: false,
            worker_dead: false,
        };
        this.load_current();
        cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(100))
                .await;
            if this.update(cx, |this, cx| this.drain(cx)).is_err() {
                break;
            }
        })
        .detach();
        let _ = window;
        this
    }

    /// Queue an Engine op into a named slot; the reply overwrites it.
    pub fn call(&mut self, slot: impl Into<String>, op: &'static str, params: Value) {
        let slot = slot.into();
        self.slots.insert(slot.clone(), Slot::Loading);
        if self
            .worker
            .commands
            .send(Command::Rpc { slot, op, params })
            .is_err()
        {
            self.worker_dead = true;
            self.error = Some("Соединение с Engine завершено. Перезапустите приложение.".into());
        }
    }

    /// Queue a GET /v1/<path> status surface into a named slot.
    pub fn status(&mut self, slot: impl Into<String>, path: &'static str) {
        let slot = slot.into();
        self.slots.insert(slot.clone(), Slot::Loading);
        if self
            .worker
            .commands
            .send(Command::Get { slot, path })
            .is_err()
        {
            self.worker_dead = true;
            self.error = Some("Соединение с Engine завершено. Перезапустите приложение.".into());
        }
    }

    /// Mutation op: on success reloads the current view (Vue Manager does the
    /// same refresh-after-write), on failure shows the Engine error.
    pub fn action(&mut self, op: &'static str, params: Value) {
        if self.action_busy {
            return;
        }
        self.action_busy = true;
        self.notice = None;
        self.call("@action", op, params);
    }

    /// Destructive op behind the confirm modal.
    pub fn ask_confirm(
        &mut self,
        title: impl Into<String>,
        body: impl Into<String>,
        op: &'static str,
        params: Value,
        cx: &mut Context<Self>,
    ) {
        self.confirm = Some(Confirm {
            title: title.into(),
            body: body.into(),
            op,
            params,
        });
        cx.notify();
    }

    pub fn load_current(&mut self) {
        self.notice = None;
        views::load(self.view, self);
    }

    pub fn set_view(&mut self, view: View, cx: &mut Context<Self>) {
        if self.view == view {
            return;
        }
        self.view = view;
        self.detail = None;
        self.disclosure = None;
        self.confirm = None;
        self.load_current();
        cx.notify();
    }

    /// Lazily create a text input keyed per view field.
    pub fn input(
        &mut self,
        key: &str,
        placeholder: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        if let Some(state) = self.inputs.get(key) {
            return state.clone();
        }
        let state = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
        self.inputs.insert(key.to_string(), state.clone());
        state
    }

    pub fn input_value(&self, key: &str, cx: &Context<Self>) -> String {
        self.inputs
            .get(key)
            .map(|s| s.read(cx).value().trim().to_string())
            .unwrap_or_default()
    }

    pub fn slot(&self, key: &str) -> Option<&Slot> {
        self.slots.get(key)
    }

    /// Ready slot payload or Null — views stay total over missing data.
    pub fn data(&self, key: &str) -> Value {
        match self.slots.get(key) {
            Some(Slot::Ready(v)) => v.clone(),
            _ => Value::Null,
        }
    }

    fn drain(&mut self, cx: &mut Context<Self>) {
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
            if reply.slot == "@action" {
                self.action_busy = false;
                match reply.result {
                    Ok(_) => {
                        self.notice = Some("Выполнено.".into());
                        views::load(self.view, self);
                    }
                    Err(e) => self.error = Some(e),
                }
            } else if reply.slot == "disclosure" {
                // packages.disclosure → consent overlay before install.
                match reply.result {
                    Ok(v) => self.disclosure = Some(v),
                    Err(e) => self.error = Some(e),
                }
            } else if reply.slot == "store.ext" {
                // store.external_url → open in the system browser.
                match reply.result {
                    Ok(v) => {
                        let url = v
                            .get("url")
                            .and_then(Value::as_str)
                            .map(str::to_string)
                            .unwrap_or_else(|| v.as_str().unwrap_or_default().to_string());
                        if url.is_empty() {
                            self.error = Some("Engine не вернул ссылку маркетплейса.".into());
                        } else if let Err(e) = crate::engine::open_url(&url) {
                            self.error = Some(e);
                        }
                    }
                    Err(e) => self.error = Some(e),
                }
            } else {
                self.slots.insert(
                    reply.slot,
                    match reply.result {
                        Ok(v) => {
                            if self.error.is_some() {
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
            }
            cx.notify();
        }
    }
}
