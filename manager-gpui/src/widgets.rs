//! Shared chrome: sidebar, titlebar, Engine banner — the pieces bound to
//! `ManagerApp`/`View` stay here. View primitives, JSON accessors and window
//! controls live in `kosmos_gpui_kit` (re-exported for the views). Modals
//! live in `modals.rs`. Shell pieces come from `imago_gpui::chrome`.
use ::gpui::{prelude::*, *};
use gpui_component::Sizable;
use imago_gpui::chrome::{self, SIDEBAR_W};

pub use kosmos_gpui_kit::widgets::*;

use crate::app::ManagerApp;
use crate::views;
use kosmos_gpui_kit::theme::*;

// --- Chrome -----------------------------------------------------------------

pub fn render_sidebar(
    app: &mut ManagerApp,
    progress: f32,
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
                }),
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
                        .child(
                            chrome::sidebar_titlebar().child(
                                div()
                                    .ml(px(40.))
                                    .flex_1()
                                    .h_full()
                                    .window_control_area(WindowControlArea::Drag),
                            ),
                        )
                        .child(nav),
                ),
        )
}

pub fn render_titlebar(
    app: &ManagerApp,
    sidebar_progress: f32,
    cx: &mut Context<ManagerApp>,
) -> impl IntoElement {
    let left = 52.0 + (11.0 - 52.0) * sidebar_progress;
    chrome::titlebar()
        .p_0()
        .child(div().w(px(left)).h_full().flex_none())
        .child(
            div()
                .flex_1()
                .h_full()
                .min_w_0()
                .flex()
                .items_center()
                .gap_1p5()
                .window_control_area(WindowControlArea::Drag)
                .child(app.view.icon().with_size(px(18.)))
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(FontWeight::MEDIUM)
                        .child(app.view.label()),
                ),
        )
        .child(
            div()
                .id("refresh")
                .h_7()
                .px_2()
                .flex()
                .items_center()
                .rounded_md()
                .text_size(px(12.))
                .cursor_pointer()
                .hover(|style| style.bg(fade(FG(), 0.08)))
                .child("Обновить")
                .on_click(cx.listener(|this, _, _, cx| {
                    this.load_current();
                    cx.notify();
                }))
                .role(Role::Button)
                .aria_label("Обновить")
                .accessibility_id("refresh"),
        )
        // Same frameless min/close row as kosmos_gpui_kit's helper, but with
        // AccessKit role + Russian names (KOS-142).
        .child(chrome::window_controls())
}

pub fn render_sidebar_toggle(open: bool, cx: &mut Context<ManagerApp>) -> impl IntoElement {
    let weak = cx.weak_entity();
    div()
        .id("sidebar-toggle")
        .absolute()
        .top_0()
        .left(px(12.))
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
    let text = if app.error.is_some() {
        app.error.clone().unwrap_or_default()
    } else {
        "Запрос выполняется…".into()
    };
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
        .when(app.error.is_some(), |d| {
            d.child(
                imago_gpui::button::secondary("retry")
                    .label("Обновить")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.error = None;
                        this.load_current();
                        cx.notify();
                    })),
            )
        })
}
