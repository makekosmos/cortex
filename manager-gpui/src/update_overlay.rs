//! Full-window update overlay (KOS-355, round 2): the Mundus petal mark
//! rendered as Paper "Liquid Metal" — a faithful CPU port of the
//! `liquid-metal` fragment shader evaluated per frame and clipped to the
//! logo's alpha — floating over an opaque shell, state-driven content
//! underneath. No badge or plate behind the mark.
//!
//! The machine is deliberately tiny and pure: `resolve` maps
//! (snoozed, was_open, Engine `updater.status.state`) to the visible state.
//! Engine drives the truth; the overlay only decides whether to show itself.
//! `Failed` is reachable only while the overlay is already open — a failed
//! background check must never take over the whole window unprompted.
use ::gpui::{prelude::*, *};
use serde_json::json;
use std::sync::Arc;

use crate::app::ManagerApp;
use crate::button;
use crate::liquid_metal::{self, LogoField};
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

/// The petal mark itself, filled with animated liquid metal. One frame is a
/// few dozen KB of CPU-shaded pixels at 168×165 — cheap enough for vsync-rate
/// repaints while the overlay is on screen.
fn logo(field: Option<&Arc<LogoField>>, elapsed: f32) -> Stateful<Div> {
    let mut mark = div()
        .id("update-overlay-logo")
        .debug_selector(|| "update-overlay-logo".into())
        .relative()
        .top(px(bounce_offset(elapsed)))
        .size(px(160.));
    if let Some(field) = field {
        mark = mark.child(img(liquid_metal::frame(field, elapsed)).size_full());
    } else {
        // Fallback if the SVG mask could not be rasterized: plain glyph.
        mark = mark.flex().items_center().justify_center().child(
            gpui_component::Icon::default()
                .path("icons/mundus.svg")
                .size(px(140.))
                .text_color(c(FG())),
        );
    }
    mark
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
    // The metal flow needs continuous frames only while the overlay is on
    // screen; Hidden requests nothing, so idle CPU stays at zero.
    window.request_animation_frame();

    if app.update_logo.is_none() {
        app.update_logo = LogoField::build(&cx.svg_renderer());
    }

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
        OverlayState::Failed => "Повторить",
        _ => "Обновить",
    };
    let busy = app.action_busy;

    let mut body = div()
        .flex()
        .flex_col()
        .items_center()
        .gap_4()
        .child(logo(app.update_logo.as_ref(), elapsed))
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

    // Actions stack vertically, equal width, centered: accent on top.
    let mut actions = div().flex().flex_col().items_center().gap_2().w(px(240.));
    if state != OverlayState::Downloading {
        actions = actions.child(
            div()
                .id("update-overlay-primary")
                .debug_selector(|| "update-overlay-primary".into())
                .w_full()
                .child(
                    button::primary("update-overlay-primary-btn")
                        .w_full()
                        .label(primary_label)
                        .disabled(busy)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.action(primary_op(state), json!({}));
                            cx.notify();
                        })),
                ),
        );
    }
    actions = actions.child(
        div()
            .id("update-overlay-later")
            .debug_selector(|| "update-overlay-later".into())
            .w_full()
            .child(
                button::secondary("update-overlay-later-btn")
                    .w_full()
                    .label("Позже")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.update_snoozed = true;
                        this.update_overlay = OverlayState::Hidden;
                        cx.notify();
                    })),
            ),
    );
    body = body.child(actions);

    div()
        .id("update-overlay")
        .debug_selector(|| "update-overlay".into())
        .absolute()
        .size_full()
        // Opaque takeover: the shell underneath must not bleed through.
        .bg(c(BG()))
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
