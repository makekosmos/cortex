//! Dictation pill — always-on-top overlay shown while a recording session is
//! active (desktop/electron/dictation-pill.ts parity).
//!
//! What gpui-kit 0.6.2 actually supports on Windows (gpui-pre-windows):
//!   * `WindowKind::PopUp` → `WS_EX_TOOLWINDOW | WS_EX_TOPMOST` + bare
//!     `WINDOW_STYLE(0)` — topmost borderless tool window, hidden from the
//!     taskbar (Electron `alwaysOnTop` + `skipTaskbar` + `frame: false`).
//!   * `focus: false` + `show: true` → `SW_SHOWNOACTIVATE` placement, so the
//!     window appears without stealing foreground (Electron `showInactive`).
//!     This is what keeps the dictation target app focused for Ctrl+V inject.
//!   * `window_background: Transparent` → DWM transparent composition.
//!
//! Gaps vs the Electron pill: there is no per-window hide/show in the
//! PlatformWindow trait, so the window is opened on record start and closed
//! via `Window::remove_window`; the waveform (renderer-side AnalyserNode) has
//! no equivalent — the pill shows a static status instead. The global
//! hotkey reaches us through the Engine WS subscription
//! (`dictation_toggle_trigger` / `dictation_ptt_trigger` →
//! `ManagerApp::dictation_toggle`, same path as the view button).
use ::gpui::{prelude::*, *};
use imago_gpui::button;

use crate::app::ManagerApp;
use kosmos_gpui_kit::theme::*;

/// Electron pill is 380x126; ours is shorter — no waveform row.
const PILL_W: f32 = 380.;
const PILL_H: f32 = 64.;
/// Bottom-center of the work area, like the Electron pill.
const BOTTOM_MARGIN: f32 = 100.;

/// Recording session phase mirrored from `ManagerApp` so the pill and the
/// Диктовка view render the same machine.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PillPhase {
    /// `dictation.capture.start` in flight.
    Starting,
    /// Engine-owned WASAPI capture is live; pill window is visible.
    Recording,
    /// Pill closed, `capture.stop` → `speech.transcribe` chain in flight.
    Processing,
}

impl PillPhase {
    pub fn label(self) -> &'static str {
        match self {
            PillPhase::Starting => "Запуск записи…",
            PillPhase::Recording => "Идёт запись — говорите",
            PillPhase::Processing => "Распознаю…",
        }
    }
}

pub struct DictationPill {
    manager: Entity<ManagerApp>,
    _subscription: Subscription,
}

impl DictationPill {
    fn new(manager: Entity<ManagerApp>, cx: &mut Context<Self>) -> Self {
        let subscription = cx.observe(&manager, |_, _, cx| cx.notify());
        Self {
            manager,
            _subscription: subscription,
        }
    }
}

/// Open the pill bottom-center of the primary display's work area. Returns
/// `None` when no display is available or window creation failed — callers
/// fall back to the in-view status instead of crashing the app.
pub fn open(manager: Entity<ManagerApp>, cx: &mut App) -> Option<WindowHandle<DictationPill>> {
    let display = cx.primary_display()?;
    let area = display.visible_bounds();
    let origin = point(
        area.center().x - px(PILL_W / 2.),
        area.origin.y + area.size.height - px(PILL_H + BOTTOM_MARGIN),
    );
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin,
                size: size(px(PILL_W), px(PILL_H)),
            })),
            titlebar: None,
            focus: false,
            show: true,
            kind: WindowKind::PopUp,
            is_movable: false,
            is_resizable: false,
            is_minimizable: false,
            window_background: WindowBackgroundAppearance::Transparent,
            ..Default::default()
        },
        move |_, cx| cx.new(|cx| DictationPill::new(manager, cx)),
    )
    .ok()
}

impl Render for DictationPill {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let phase = self
            .manager
            .read_with(cx, |app, _| app.pill_phase)
            .unwrap_or(PillPhase::Starting);
        let recording = matches!(phase, PillPhase::Recording);
        div()
            .size_full()
            .rounded_full()
            .border_1()
            .border_color(c(BORDER()))
            .bg(c(POPOVER()))
            .flex()
            .items_center()
            .gap_3()
            .px_4()
            .font_family("Inter")
            .text_color(c(FG()))
            .child(
                div()
                    .w(px(10.))
                    .h(px(10.))
                    .flex_none()
                    .rounded_full()
                    .bg(c(if recording { DESTRUCTIVE() } else { WARN() })),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(13.))
                    .font_weight(FontWeight::MEDIUM)
                    .whitespace_nowrap()
                    .overflow_hidden()
                    .child(phase.label()),
            )
            .child(
                button::ghost("pill-cancel")
                    .label("Отмена")
                    .on_click(cx.listener(|this, _, _, cx| {
                        let manager = this.manager.clone();
                        manager.update(cx, |app, cx| app.dictation_cancel(cx));
                    })),
            )
            .when(recording, |el| {
                el.child(
                    button::primary("pill-stop")
                        .label("Стоп")
                        .on_click(cx.listener(|this, _, _, cx| {
                            let manager = this.manager.clone();
                            manager.update(cx, |app, cx| app.dictation_toggle(cx));
                        })),
                )
            })
    }
}
