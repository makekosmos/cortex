//! Root entity: sidebar nav, per-view data slots, Engine error banner,
//! confirmation modal. Every Engine answer lands in `slots` keyed by a
//! string the issuing view chose — views render whatever arrived.
use std::collections::HashMap;
use std::sync::mpsc::TryRecvError;
use std::time::Instant;

use ::gpui::{prelude::*, *};
use gpui_component::input::InputState;
use serde_json::{json, Value};

use crate::fps::FpsOverlay;
use crate::pill::{DictationPill, PillPhase};
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
    /// Dictation pill overlay window while a recording session is active.
    /// `WindowKind::PopUp` — no hide API exists, so the window is closed
    /// (`remove_window`) rather than hidden.
    pub pill: Option<WindowHandle<DictationPill>>,
    /// Pill session phase (Starting/Recording/Processing) — rendered by both
    /// the pill window and the Диктовка view.
    pub pill_phase: Option<PillPhase>,
    /// `captureId` of the live `dictation.capture.start` session.
    pill_capture: Option<String>,
    /// `durationMs` from `capture.stop` — merged into the transcribe result
    /// for the result card (the transcribe reply doesn't carry it).
    pill_last_duration_ms: f64,
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
            pill: None,
            pill_phase: None,
            pill_capture: None,
            pill_last_duration_ms: 0.0,
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

    /// Queue the composite usage report (analytics + icon resolution) into a
    /// named slot; the reply overwrites it.
    pub fn usage_report(&mut self, slot: impl Into<String>) {
        let slot = slot.into();
        self.slots.insert(slot.clone(), Slot::Loading);
        if self
            .worker
            .commands
            .send(Command::UsageReport { slot })
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

    /// Worker command that doesn't map to a view-data slot (pill session
    /// control ops are intercepted by name in `drain`).
    fn send_command(&mut self, command: Command) {
        if self.worker.commands.send(command).is_err() {
            self.worker_dead = true;
            self.error = Some("Соединение с Engine завершено. Перезапустите приложение.".into());
        }
    }

    /// Dictation pill toggle — view button and the pill's own Стоп button.
    /// `toggleDictation` parity: re-entry while Starting/Processing is ignored.
    pub fn dictation_toggle(&mut self, cx: &mut Context<Self>) {
        match self.pill_phase {
            None => self.dictation_begin(cx),
            Some(PillPhase::Recording) => self.dictation_finish(cx),
            Some(PillPhase::Starting | PillPhase::Processing) => {}
        }
    }

    /// Cancel the session from any phase: close the pill, terminate the
    /// capture session and reset the backend state machine.
    pub fn dictation_cancel(&mut self, cx: &mut Context<Self>) {
        self.close_pill(cx);
        self.pill_phase = None;
        let capture_id = self.pill_capture.take();
        self.send_command(Command::DictationCancel {
            slot: "dictation.pill.cancel".into(),
            capture_id,
        });
        cx.notify();
    }

    /// Idle → Starting: open the overlay immediately (it renders "Запуск
    /// записи…"), then ask the Engine to capture the foreground HWND and start
    /// WASAPI capture.
    fn dictation_begin(&mut self, cx: &mut Context<Self>) {
        if self.pill.is_none() {
            self.pill = crate::pill::open(cx.entity(), cx);
        }
        self.pill_phase = Some(PillPhase::Starting);
        self.send_command(Command::DictationStart {
            slot: "dictation.pill.start".into(),
        });
        cx.notify();
    }

    /// Recording → Processing: close the pill BEFORE `capture.stop` so the OS
    /// foreground returns to the target window — the transcribe step
    /// re-captures that HWND for `injectMode: auto_paste` delivery.
    fn dictation_finish(&mut self, cx: &mut Context<Self>) {
        self.close_pill(cx);
        self.pill_phase = Some(PillPhase::Processing);
        match self.pill_capture.take() {
            Some(capture_id) => self.send_command(Command::DictationStop {
                slot: "dictation.pill.stop".into(),
                capture_id,
            }),
            None => self.send_command(Command::DictationCancel {
                slot: "dictation.pill.cancel".into(),
                capture_id: None,
            }),
        }
        cx.notify();
    }

    fn close_pill(&mut self, cx: &mut Context<Self>) {
        if let Some(handle) = self.pill.take() {
            handle
                .update(cx, |_, window, _| window.remove_window())
                .ok();
        }
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
            } else if reply.slot == "dictation.pill.start" {
                match reply.result {
                    Ok(v) => {
                        let capture_id = v
                            .get("captureId")
                            .and_then(Value::as_str)
                            .map(str::to_string);
                        // Cancel during Starting: the pill is already gone —
                        // stop the just-started session instead of resurrecting.
                        if self.pill_phase.is_none() {
                            self.send_command(Command::DictationCancel {
                                slot: "dictation.pill.cancel".into(),
                                capture_id,
                            });
                        } else if let Some(id) = capture_id {
                            self.pill_capture = Some(id);
                            self.pill_phase = Some(PillPhase::Recording);
                        } else {
                            self.pill_phase = None;
                            self.close_pill(cx);
                            self.error = Some("Engine не вернул captureId".into());
                        }
                    }
                    Err(e) => {
                        self.pill_phase = None;
                        self.pill_capture = None;
                        self.close_pill(cx);
                        self.error = Some(e);
                    }
                }
            } else if reply.slot == "dictation.pill.stop" {
                match reply.result {
                    Ok(v) => {
                        let audio_b64 = v
                            .get("audioB64")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string();
                        // speech.transcribe's reply has no durationMs — carry it
                        // over so the result card shows the record length.
                        self.pill_last_duration_ms =
                            v.get("durationMs").and_then(Value::as_f64).unwrap_or(0.0);
                        self.send_command(Command::DictationTranscribe {
                            slot: "dictation.pill.result".into(),
                            audio_b64,
                            duration_sec: self.pill_last_duration_ms / 1000.0,
                        });
                    }
                    Err(e) => {
                        self.pill_phase = None;
                        self.error = Some(e);
                    }
                }
            } else if reply.slot == "dictation.pill.result" {
                self.pill_phase = None;
                match reply.result {
                    Ok(mut v) => {
                        if let Some(obj) = v.as_object_mut() {
                            obj.insert(
                                "durationMs".into(),
                                Value::from(self.pill_last_duration_ms),
                            );
                        }
                        self.slots.insert("dictation.result".into(), Slot::Ready(v));
                        if self.error.is_some() {
                            self.error = None;
                        }
                        // Post-session refresh: state machine + stats + очередь
                        // (same refresh-after-write as `action`).
                        self.call("dictation.state", "dictation.get_state", json!({}));
                        self.call("dictation.stats", "dictation.get_stats", json!({}));
                        self.call("dictation.pending", "dictation.list_pending", json!({}));
                    }
                    Err(e) => self.error = Some(e),
                }
            } else if reply.slot == "dictation.pill.cancel" {
                self.pill_phase = None;
                self.pill_capture = None;
                if let Err(e) = reply.result {
                    self.error = Some(e);
                }
                if matches!(self.view, View::Dictation) {
                    self.call("dictation.state", "dictation.get_state", json!({}));
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
        // Engine broadcast events from the WS subscription (worker thread →
        // this UI-thread drain). A dead events thread degrades the hotkey
        // silently — the RPC surface keeps working, so no error banner.
        loop {
            let event = match self.worker.events.try_recv() {
                Ok(event) => event,
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
            };
            self.handle_engine_event(event, cx);
            cx.notify();
        }
    }

    /// desktop/electron/dictation-pill.ts parity: the Rust WH_KEYBOARD_LL
    /// hook emits `dictation_toggle_trigger` (toggle mode) and
    /// `dictation_ptt_trigger {phase}` (PTT mode — both phases route into the
    /// same toggle call the view button and pill Стоп use). State/progress
    /// events refresh the matching Диктовка slots.
    fn handle_engine_event(&mut self, event: Value, cx: &mut Context<Self>) {
        let name = event
            .get("event")
            .and_then(Value::as_str)
            .unwrap_or_default();
        match name {
            "dictation_toggle_trigger" | "dictation_ptt_trigger" => {
                self.dictation_toggle(cx);
            }
            "dictation_state_changed" | "dictation.state_changed" | "dictation_config_changed" => {
                self.call("dictation.state", "dictation.get_state", json!({}));
            }
            "dictation_stats_changed" => {
                self.call("dictation.stats", "dictation.get_stats", json!({}));
            }
            "dictation_pending_changed" => {
                self.call("dictation.pending", "dictation.list_pending", json!({}));
            }
            // Progress ticks stream per chunk — stash the payload for the
            // view rather than re-issuing RPCs; started resets the slot and
            // complete/failed clear it while refreshing the model list.
            "dictation_local_model_download_progress"
            | "dictation_local_model_download_started" => {
                self.slots
                    .insert("dictation.download".into(), Slot::Ready(event));
            }
            "dictation_local_model_download_complete" | "dictation_local_model_download_failed" => {
                self.slots.remove("dictation.download");
                self.call("dictation.local", "dictation.local_status", json!({}));
                self.call("dictation.models", "dictation.list_local_models", json!({}));
            }
            // Hotkey-capture feed for a future capture row in Диктовка
            // (`dictation.begin_hotkey_capture` isn't exposed there yet).
            "dictation_capture_key" | "dictation_capture_cancelled" => {
                self.slots
                    .insert("dictation.capture".into(), Slot::Ready(event));
            }
            _ => {}
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
