//! Full-window update overlay (KOS-355) — Grok-bot style: a centered Mundus
//! badge with an animated metallic sweep and a gentle bounce over a dimmed
//! shell, state-driven content underneath.
//!
//! The machine is deliberately tiny and pure: `resolve` maps
//! (snoozed, was_open, Engine `updater.status.state`) to the visible state.
//! Engine drives the truth; the overlay only decides whether to show itself.
//! `Failed` is reachable only while the overlay is already open — a failed
//! background check must never take over the whole window unprompted.
use ::gpui::{prelude::*, *};
use serde_json::json;

use crate::app::ManagerApp;
use crate::button;
use crate::theme::*;
use mundus_gpui_kit::fields::{vnum, vopt, vstr};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayState {
    Hidden,
    /// Engine reports `available` — offer «Обновить» / «Позже».
    Offer,
    /// Engine reports `downloading` — determinate bar from `percent` (real
    /// bytes / manifest size, emitted by `runtime/src/updater`).
    Downloading,
    /// Engine reports `downloaded` — «Обновить» now installs + relaunches.
    Ready,
    /// Download/install failed while the overlay was open — retry offered.
    Failed,
}

impl OverlayState {
    pub fn visible(self) -> bool {
        self != OverlayState::Hidden
    }
}

/// `was_open` gates `Failed`: an unattended background `error` (e.g. the
/// startup feed check on a machine with no network) stays silent in the
/// About card instead of hijacking the window.
pub fn resolve(snoozed: bool, was_open: bool, engine_state: &str) -> OverlayState {
    if snoozed {
        return OverlayState::Hidden;
    }
    match engine_state {
        "available" => OverlayState::Offer,
        "downloading" => OverlayState::Downloading,
        "downloaded" => OverlayState::Ready,
        "error" if was_open => OverlayState::Failed,
        _ => OverlayState::Hidden,
    }
}

/// Gentle bounce: ±6px sine over ~1.9s — Telegram-sticker float, not a jump.
pub fn bounce_offset(elapsed_secs: f32) -> f32 {
    (elapsed_secs * std::f32::consts::TAU / 1.9).sin() * 6.0
}

/// Shine-stripe position as a fraction of the badge width: sweeps left→right
/// in ~0.9s, then rests out of view for the remainder of the 2.4s cycle so
/// the sweep reads as a periodic glint, not a conveyor belt.
pub fn sweep_fraction(elapsed_secs: f32) -> f32 {
    let cycle = (elapsed_secs % 2.4) / 2.4;
    if cycle < 0.375 {
        cycle / 0.375 * 1.6 - 0.55
    } else {
        -0.55
    }
}

fn badge(elapsed: f32) -> Div {
    let sweep = sweep_fraction(elapsed);
    let chrome_hi = crate::theme::rgba(0xf5f8fc, 1.0);
    let chrome_lo = crate::theme::rgba(0x8fa9c4, 1.0);
    div()
        .relative()
        .top(px(bounce_offset(elapsed)))
        .size(px(112.))
        .child(
            div()
                .id("update-overlay-badge")
                .debug_selector(|| "update-overlay-badge".into())
                .absolute()
                .size_full()
                .rounded_full()
                .overflow_hidden()
                .bg(linear_gradient(
                    135.0,
                    linear_color_stop(chrome_hi, 0.0),
                    linear_color_stop(chrome_lo, 1.0),
                ))
                .border_2()
                .border_color(crate::theme::rgba(0xffffff, 0.55))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    gpui_component::Icon::default()
                        .path("icons/mundus.svg")
                        .size(px(56.))
                        .text_color(gpui::white()),
                )
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .left(relative(sweep))
                        .w(relative(0.5))
                        .bg(linear_gradient(
                            100.0,
                            linear_color_stop(crate::theme::rgba(0xffffff, 0.0), 0.0),
                            linear_color_stop(crate::theme::rgba(0xffffff, 0.6), 1.0),
                        )),
                ),
        )
}

fn primary_op(state: OverlayState) -> &'static str {
    match state {
        OverlayState::Ready => "updater.install",
        _ => "updater.download",
    }
}

pub fn render(
    app: &mut ManagerApp,
    state: OverlayState,
    window: &mut Window,
    cx: &mut Context<ManagerApp>,
) -> impl IntoElement {
    let status = app.data("upd.mundus");
    let next = vopt(&status, "newVersion").unwrap_or_default();
    let current = vstr(&status, "currentVersion");
    let elapsed = app.update_anim_start.elapsed().as_secs_f32();
    // The sweep/bounce need continuous frames only while the overlay is on
    // screen; Hidden requests nothing, so idle CPU stays at zero.
    window.request_animation_frame();

    let (title, detail) = match state {
        OverlayState::Offer => (
            "Доступно обновление".to_string(),
            if next.is_empty() {
                current
            } else {
                format!("{current} → {next}")
            },
        ),
        OverlayState::Downloading => ("Обновление Mundus".to_string(), format!("Скачиваем {next}")),
        OverlayState::Ready => (
            "Готово к установке".to_string(),
            format!("Mundus {next} скачан — установим и перезапустим"),
        ),
        OverlayState::Failed => (
            "Не удалось обновить".to_string(),
            vopt(&status, "message").unwrap_or_else(|| "Попробуйте ещё раз".into()),
        ),
        OverlayState::Hidden => (String::new(), String::new()),
    };

    let primary_label = match state {
        OverlayState::Ready => "Обновить",
        OverlayState::Failed => "Повторить",
        _ => "Обновить",
    };
    let busy = app.action_busy;

    let mut body = div()
        .flex()
        .flex_col()
        .items_center()
        .gap_4()
        .child(badge(elapsed))
        .child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap_1()
                .child(
                    div()
                        .text_size(ui_px(17.))
                        .line_height(ui_px(24.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title),
                )
                .child(
                    div()
                        .text_size(ui_px(13.))
                        .line_height(ui_px(18.))
                        .text_color(c(MUTED_FG()))
                        .child(detail),
                ),
        );

    if state == OverlayState::Downloading {
        let percent = vnum(&status, "percent").clamp(0.0, 100.0);
        body = body.child(
            div()
                .w(px(280.))
                .flex()
                .flex_col()
                .gap_1p5()
                .items_center()
                .child(
                    div()
                        .text_size(ui_px(12.))
                        .text_color(c(MUTED_FG()))
                        .child(format!("{percent:.0}%")),
                )
                .child(
                    div()
                        .id("update-overlay-progress")
                        .debug_selector(|| "update-overlay-progress".into())
                        .h(px(6.))
                        .w_full()
                        .rounded_full()
                        .bg(fade(FG(), 0.12))
                        .child(
                            div()
                                .h_full()
                                .w(relative(percent as f32 / 100.0))
                                .rounded_full()
                                .bg(c(ACCENT())),
                        ),
                ),
        );
    }

    body = body.child(
        div()
            .flex()
            .gap_2()
            .items_center()
            .when(state != OverlayState::Downloading, |row| {
                row.child(
                    div()
                        .id("update-overlay-primary")
                        .debug_selector(|| "update-overlay-primary".into())
                        .child(
                            button::primary("update-overlay-primary-btn")
                                .label(primary_label)
                                .disabled(busy)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.action(primary_op(state), json!({}));
                                    cx.notify();
                                })),
                        ),
                )
            })
            .child(
                div()
                    .id("update-overlay-later")
                    .debug_selector(|| "update-overlay-later".into())
                    .child(
                        button::secondary("update-overlay-later-btn")
                            .label("Позже")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.update_snoozed = true;
                                this.update_overlay = OverlayState::Hidden;
                                cx.notify();
                            })),
                    ),
            ),
    );

    div()
        .id("update-overlay")
        .debug_selector(|| "update-overlay".into())
        .absolute()
        .size_full()
        .bg(fade(0x000000, 0.55))
        .flex()
        .items_center()
        .justify_center()
        .occlude()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(body)
        .into_any_element()
}

#[cfg(test)]
#[path = "update_overlay_tests.rs"]
mod tests;
