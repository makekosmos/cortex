//! Shared chrome: sidebar, titlebar, Engine banner — the pieces bound to
//! `ManagerApp`/`View` stay here. View primitives, JSON accessors and window
//! controls live in `mundus_gpui_kit` (re-exported for the views). Modals
//! live in `modals.rs`. Shell pieces come from `imago_gpui::chrome`.
use ::gpui::{prelude::*, *};
#[cfg(target_os = "macos")]
use gpui_component::InteractiveElementExt;
use imago_gpui::chrome::{self, SIDEBAR_W};

pub use crate::button::{btn, btn_id};
pub use crate::page_layout::{
    card, empty, input_field, kv, page_sections, page_stack, row, row_copy, section, section_group,
};
pub use crate::toggle::toggle;
pub use mundus_gpui_kit::widgets::*;

use crate::app::ManagerApp;
use crate::theme::*;
use crate::views;

#[cfg(test)]
#[path = "chrome_tests.rs"]
mod chrome_tests;

// --- Store/list entries ------------------------------------------------------

/// First letter placeholder for a missing icon — the same neutral badge the
/// usage table renders when `app_index.icon_path` has nothing cached.
fn icon_letter(name: &str) -> String {
    name.chars()
        .find(|ch| ch.is_alphanumeric())
        .unwrap_or('?')
        .to_uppercase()
        .to_string()
}

/// Icon from an on-disk image path (`icon_path` fields the Engine serves).
pub fn icon_file(path: Option<String>) -> Option<ImageSource> {
    path.filter(|p| !p.is_empty())
        .map(|p| ImageSource::from(std::path::PathBuf::from(p)))
}

/// Icon from a remote image URL (store catalog `icon_url`).
pub fn icon_url(url: Option<String>) -> Option<ImageSource> {
    url.filter(|u| !u.is_empty()).map(ImageSource::from)
}

/// 20px icon slot: the letter badge always renders underneath, so a missing
/// or unloadable image degrades to a neutral placeholder — never an empty
/// hole in the row.
pub fn app_icon(source: Option<ImageSource>, name: &str) -> Div {
    let mut icon = div()
        .size(px(32.))
        .flex_none()
        .overflow_hidden()
        .rounded_md();
    if let Some(source) = source {
        icon = icon.child(img(source).size(px(32.)));
    } else {
        icon = icon
            .flex()
            .items_center()
            .justify_center()
            .text_size(ui_px(13.))
            .text_color(c(MUTED_FG()))
            .child(icon_letter(name));
    }
    icon
}

/// Icon + primary display name + muted secondary caption (usually the
/// package id) — the shared row head for every store-style list.
pub fn entry_row(icon: Option<ImageSource>, title: String, caption: String) -> Div {
    div()
        .w_full()
        .min_w_0()
        .min_h(px(36.))
        .flex()
        .items_center()
        .gap(px(crate::page_layout::GAP))
        .child(app_icon(icon, &title))
        .child(row_copy(title, caption))
}

// --- Chrome -----------------------------------------------------------------

/// Keep the overlay toggle past native macOS traffic lights. Fullscreen hides
/// them, so reclaim the inset (same layout rule as Zeron's titlebar cluster).
fn toggle_left(is_macos: bool, fullscreen: bool) -> f32 {
    if is_macos && !fullscreen {
        88.0
    } else {
        12.0
    }
}

fn drag_inset(is_macos: bool, fullscreen: bool) -> f32 {
    toggle_left(is_macos, fullscreen) + 40.0
}

fn content_inset(progress: f32, is_macos: bool, fullscreen: bool) -> f32 {
    let closed = drag_inset(is_macos, fullscreen);
    closed + (11.0 - closed) * progress.clamp(0.0, 1.0)
}

pub fn render_sidebar(
    app: &mut ManagerApp,
    progress: f32,
    window: &Window,
    cx: &mut Context<ManagerApp>,
) -> impl IntoElement {
    let mut nav = chrome::sidebar_body().gap(px(16.));
    for views in views::NAV_GROUPS {
        let mut group = div().flex().flex_col().flex_none().gap(px(1.));
        for view in *views {
            let active = app.view == *view;
            let weak = cx.weak_entity();
            group = group.child(
                chrome::sidebar_item(
                    SharedString::from(format!("nav-{:?}", view)),
                    view.icon(),
                    view.label(),
                    active,
                )
                .on_click(move |_, _, cx| {
                    weak.update(cx, |this, cx| this.set_view(*view, cx)).ok();
                })
                .into_element()
                .text_size(ui_px(13.))
                .line_height(ui_px(15.)),
            );
        }
        nav = nav.child(group);
    }
    div()
        .w(px(SIDEBAR_W * progress))
        .h_full()
        .flex_none()
        .overflow_hidden()
        .relative()
        .child(
            div()
                .absolute()
                .top_0()
                .left_0()
                .bottom_0()
                .w(px(SIDEBAR_W))
                .child(
                    chrome::sidebar()
                        .bg(sidebar_fill())
                        .child(
                            chrome::sidebar_titlebar().child(
                                div()
                                    .ml(px(drag_inset(
                                        cfg!(target_os = "macos"),
                                        window.is_fullscreen(),
                                    ) - 12.0))
                                    .flex_1()
                                    .h_full()
                                    .window_control_area(WindowControlArea::Drag),
                            ),
                        )
                        .child(nav),
                ),
        )
}

pub fn render_titlebar(sidebar_progress: f32, window: &Window) -> impl IntoElement {
    let left = content_inset(
        sidebar_progress,
        cfg!(target_os = "macos"),
        window.is_fullscreen(),
    );
    #[allow(unused_mut)]
    let mut drag = div()
        .id("titlebar-drag")
        .flex_1()
        .h_full()
        .min_w_0()
        .flex()
        .items_center()
        .window_control_area(WindowControlArea::Drag);
    #[cfg(target_os = "macos")]
    {
        drag = drag.on_double_click(|_, window, _| window.titlebar_double_click());
    }
    chrome::titlebar()
        .bg(if is_glass() {
            crate::theme::rgba(0, 0.)
        } else {
            c(BG())
        })
        .p_0()
        .child(div().w(px(left)).h_full().flex_none())
        .child(drag)
        // macOS owns its native traffic lights; do not duplicate min/close.
        .when(cfg!(target_os = "macos"), |bar| bar.pr_3())
        .when(!cfg!(target_os = "macos"), |bar| {
            bar.child(chrome::window_controls())
        })
}

pub fn render_sidebar_toggle(
    open: bool,
    window: &Window,
    cx: &mut Context<ManagerApp>,
) -> impl IntoElement {
    let weak = cx.weak_entity();
    div()
        .id("sidebar-toggle")
        .absolute()
        .top_0()
        .left(px(toggle_left(
            cfg!(target_os = "macos"),
            window.is_fullscreen(),
        )))
        .h(px(chrome::TITLEBAR_H))
        .flex()
        .items_center()
        .child(
            div()
                .id("sidebar-toggle-button")
                .w(px(28.))
                .h(px(28.))
                .grid()
                .items_center()
                .justify_center()
                .rounded_md()
                .text_color(fade(FG(), 0.6))
                .hover(|button| button.bg(fade(FG(), 0.08)).text_color(c(FG())))
                .child(
                    svg()
                        .path("icons/sidebar-left.svg")
                        .size(px(18.))
                        .text_color(fade(FG(), 0.6)),
                )
                .on_click(move |_, _, cx| {
                    let _ = weak.update(cx, |this, cx| {
                        this.sidebar_target = if this.sidebar_target > 0.5 { 0.0 } else { 1.0 };
                        this.sidebar_stamp = std::time::Instant::now();
                        cx.notify();
                    });
                })
                .role(Role::Switch)
                .aria_label("Боковая панель")
                .aria_toggled(if open { Toggled::True } else { Toggled::False })
                .accessibility_id("sidebar-toggle"),
        )
}

pub fn render_banner(
    app: &ManagerApp,
    sidebar_progress: f32,
    cx: &mut Context<ManagerApp>,
) -> impl IntoElement {
    // The banner is also the success notice (e.g. a completed pairing) —
    // `notice` used to be write-only, so «Выполнено.» was invisible (KOS-367).
    let is_error = app.error.is_some();
    let text = app
        .error
        .clone()
        .or_else(|| app.notice.clone())
        .unwrap_or_default();
    div()
        .absolute()
        .bottom_3()
        .left(px(SIDEBAR_W * sidebar_progress + 12.0))
        .right_3()
        .p_3()
        .rounded_md()
        .border_1()
        .border_color(c(BORDER()))
        .bg(c(POPOVER()))
        .flex()
        .items_center()
        .gap_3()
        .child(div().flex_1().text_size(px(13.)).child(text))
        .when(is_error, |d| {
            d.child(
                crate::button::secondary("retry")
                    .label("Обновить")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.error = None;
                        this.load_current();
                        cx.notify();
                    })),
            )
        })
        .when(!is_error, |d| {
            d.child(
                crate::button::secondary("dismiss-notice")
                    .label("OK")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.notice = None;
                        cx.notify();
                    })),
            )
        })
}
