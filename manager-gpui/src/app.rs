//! Root entity: sidebar nav, per-view data slots, Engine error banner,
//! confirmation modal. Every Engine answer lands in `slots` keyed by a
//! string the issuing view chose — views render whatever arrived.
use std::collections::HashMap;
use std::sync::mpsc::TryRecvError;
use std::time::Instant;

use ::gpui::{prelude::*, *};
use gpui_component::input::InputState;
use serde_json::Value;

use crate::fps::FpsOverlay;
use crate::views::{self, StoreTab, View};
use crate::worker::{Command, Worker};

pub use kosmos_gpui_kit::fields::Slot;

pub struct Confirm {
    pub title: String,
    pub body: String,
    pub op: &'static str,
    pub params: Value,
}

pub struct ManagerApp {
    pub view: View,
    pub sidebar_t: f32,
    pub sidebar_target: f32,
    pub sidebar_stamp: Instant,
    pub theme_mode: u8,
    pub theme_idx: usize,
    pub dev_fps: bool,
    pub fps_view: Entity<FpsOverlay>,
    applied_theme: Option<(u8, usize, bool)>,
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
        let data_dir = kosmos_gpui_kit::engine::data_dir().ok();
        let mut this = Self {
            view: View::Data,
            sidebar_t: 1.0,
            sidebar_target: 1.0,
            sidebar_stamp: Instant::now(),
            theme_mode: 1,
            theme_idx: 0,
            dev_fps: std::env::var("MANAGER_FPS").is_ok(),
            fps_view: {
                let manager = cx.weak_entity();
                cx.new(|_| FpsOverlay::new(manager))
            },
            applied_theme: None,
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

    pub fn sync_theme(&mut self, window: &Window, cx: &mut Context<Self>) {
        let system_dark = matches!(
            window.appearance(),
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        );
        let dark = match self.theme_mode {
            0 => false,
            2 => system_dark,
            _ => true,
        };
        let selected = (self.theme_mode, self.theme_idx, dark);
        if self.applied_theme == Some(selected) {
            return;
        }
        self.applied_theme = Some(selected);
        imago_gpui::theme::set_mode(dark);
        imago_gpui::theme::set_theme(self.theme_idx);
        imago_gpui::theme::apply(cx);
        cx.set_window_appearance(match self.theme_mode {
            0 => Some(WindowAppearance::Light),
            1 => Some(WindowAppearance::Dark),
            _ => None,
        });
    }

    pub fn sidebar_progress(&mut self, window: &Window) -> f32 {
        let now = Instant::now();
        let dt = now.duration_since(self.sidebar_stamp).as_secs_f32();
        self.sidebar_stamp = now;
        self.sidebar_t = advance_sidebar(self.sidebar_t, self.sidebar_target, dt);
        if (self.sidebar_t - self.sidebar_target).abs() > 0.0005 {
            window.request_animation_frame();
        }
        imago_gpui::theme::ease_emphasized(self.sidebar_t.clamp(0.0, 1.0))
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
                        } else if let Err(e) = kosmos_gpui_kit::engine::open_url(&url) {
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

/// Exposes the named data slots to `kosmos_gpui_kit::fields::slot_or`.
impl kosmos_gpui_kit::fields::Slots for ManagerApp {
    fn slot(&self, key: &str) -> Option<&Slot> {
        self.slots.get(key)
    }
}

fn advance_sidebar(current: f32, target: f32, dt: f32) -> f32 {
    let step = dt / 0.33;
    if target > current {
        (current + step).min(target)
    } else {
        (current - step).max(target)
    }
}

#[cfg(test)]
mod tests {
    use super::advance_sidebar;

    #[test]
    fn sidebar_animation_moves_both_directions_without_overshoot() {
        assert_eq!(advance_sidebar(1.0, 0.0, 0.33), 0.0);
        assert_eq!(advance_sidebar(0.0, 1.0, 0.33), 1.0);
        assert!((advance_sidebar(1.0, 0.0, 0.165) - 0.5).abs() < 0.001);
    }
}
