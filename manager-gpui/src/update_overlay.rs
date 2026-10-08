//! Full-window update overlay (KOS-355): the Mundus petal mark rendered as
//! Paper "Liquid Metal" — a pre-baked animated WebP loop decoded once into
//! a `RenderImage` ring (see `logo_anim.rs`) — floating over an opaque
//! shell, state-driven content underneath. No badge or plate behind the
//! mark.
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
use crate::logo_anim::LogoFrames;
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

/// Displayed edge length of the mark (logical px).
const LOGO_SIZE: f32 = 100.0;
/// Produce at most one new logo frame per this interval — the paint loop
/// runs at display refresh, but a new GPU/WebP frame every ~33 ms is plenty.
const LOGO_FRAME_INTERVAL: std::time::Duration = std::time::Duration::from_millis(33);

/// The petal mark itself, filled with animated liquid metal: a ring of
/// pre-baked 256×252 frames, indexed by elapsed time — a clone of an `Arc`
/// per repaint while the overlay is on screen. The mark sits still; only
/// the metal inside it moves.
fn logo(image: Option<Arc<RenderImage>>) -> Stateful<Div> {
    let mut mark = div()
        .id("update-overlay-logo")
        .debug_selector(|| "update-overlay-logo".into())
        .size(px(LOGO_SIZE));
    if let Some(image) = image {
        mark = mark.child(img(image).size_full());
    } else {
        // Fallback if the embedded WebP could not be decoded: plain glyph.
        mark = mark.flex().items_center().justify_center().child(
            gpui_component::Icon::default()
                .path("icons/mundus.svg")
                .size(px(LOGO_SIZE * 0.7))
                .text_color(c(FG())),
        );
    }
    mark
}

/// The logo image for this repaint. With `logo-gpu` enabled the live wgpu
/// shader is tried first (rendered at displayed size × scale factor); any
/// failure or an in-flight first readback falls back to the baked WebP ring
/// (decoded on first use, so a healthy GPU path never pays for it). New
/// frames are produced at most every `LOGO_FRAME_INTERVAL`; in between (and
/// while the window is not visible) the last image is reused.
fn logo_image(app: &mut ManagerApp, window: &Window, elapsed: f32) -> Option<Arc<RenderImage>> {
    let fresh = app
        .update_logo_frame
        .as_ref()
        .is_some_and(|(at, _)| at.elapsed() < LOGO_FRAME_INTERVAL);
    if !window.is_visible() || fresh {
        return app.update_logo_frame.as_ref().map(|(_, img)| img.clone());
    }
    #[cfg(feature = "logo-gpu")]
    if let Some(img) =
        crate::logo_gpu::frame(elapsed, (LOGO_SIZE * window.scale_factor()).ceil() as u32)
    {
        #[cfg(test)]
        {
            app.update_logo_frames += 1;
        }
        app.update_logo_frame = Some((std::time::Instant::now(), img.clone()));
        return Some(img);
    }
    if app.update_logo.is_none() {
        app.update_logo = LogoFrames::shared();
    }
    let img = app.update_logo.as_ref().map(|f| f.frame(elapsed));
    if let Some(img) = &img {
        #[cfg(test)]
        {
            app.update_logo_frames += 1;
        }
        app.update_logo_frame = Some((std::time::Instant::now(), img.clone()));
    }
    img
}

fn primary_op(state: OverlayState) -> &'static str {
    match state {
        OverlayState::Ready => "updater.install",
        _ => "updater.download",
    }
}

/// Exponential approach of the displayed fill toward the polled `percent`
/// (~80 ms time constant → settles in ~0.3 s) so the bar glides between the
/// once-a-second status polls instead of jumping.
fn smoothed_fill(app: &mut ManagerApp, target: f32) -> f32 {
    let now = std::time::Instant::now();
    let dt = now
        .duration_since(app.update_fill_stamp)
        .as_secs_f32()
        .min(0.25);
    app.update_fill_stamp = now;
    app.update_fill += (target - app.update_fill) * (1.0 - (-dt / 0.08).exp());
    if (target - app.update_fill).abs() < 0.2 {
        app.update_fill = target;
    }
    app.update_fill
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
    // screen AND the window is actually presented; Hidden requests nothing,
    // so idle CPU stays at zero. `window.is_visible()` covers minimized /
    // fully occluded / other-Space windows where the platform supports it.
    let window_visible = window.is_visible();
    if window_visible {
        window.request_animation_frame();
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
        .child(logo(logo_image(app, window, elapsed)))
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

    // Actions stack vertically, equal width, centered: accent on top.
    // While downloading there are no buttons at all — the slot carries a
    // juicy filling progress bar instead (the download must not be
    // dismissible mid-flight).
    let mut actions = div().flex().flex_col().items_center().gap_2().w(px(240.));
    if state == OverlayState::Downloading {
        let percent = vnum(&status, "percent").clamp(0.0, 100.0);
        let fill = smoothed_fill(app, percent as f32);
        actions = actions
            .child(
                div()
                    .id("update-overlay-progress")
                    .debug_selector(|| "update-overlay-progress".into())
                    .h(px(12.))
                    .w_full()
                    .rounded_full()
                    .bg(fade(FG(), 0.10))
                    .child(
                        div()
                            .id("update-overlay-progress-fill")
                            .debug_selector(|| "update-overlay-progress-fill".into())
                            .relative()
                            .h_full()
                            .w(relative(fill / 100.0))
                            .rounded_full()
                            .overflow_hidden()
                            .bg(linear_gradient(
                                180.,
                                linear_color_stop(c(ACCENT()).blend(white().opacity(0.28)), 0.),
                                linear_color_stop(c(ACCENT()), 1.),
                            ))
                            .child(
                                div()
                                    .absolute()
                                    .top_0()
                                    .left_0()
                                    .right_0()
                                    .h(relative(0.5))
                                    .rounded_full()
                                    .bg(white().opacity(0.16)),
                            ),
                    ),
            )
            .child(
                div()
                    .text_size(ui_px(12.))
                    .line_height(ui_px(16.))
                    .text_color(c(MUTED_FG()))
                    .child(format!("{percent:.0}%")),
            );
    } else {
        actions = actions
            .child(
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
            )
            .child(
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
    }
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
