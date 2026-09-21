//! Shared chrome: sidebar, titlebar, Engine banner. Modals live in
//! `modals.rs`, view primitives and JSON accessors in `fields.rs`
//! (re-exported here).
use ::gpui::{prelude::*, *};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::scroll::ScrollableElement;
use gpui_component::Sizable;

pub use crate::fields::*;

use crate::app::ManagerApp;
use crate::theme::*;
use crate::views;

pub const SIDEBAR_W: f32 = 232.0;
pub const TITLEBAR_H: f32 = 40.0;

// --- Chrome -----------------------------------------------------------------

pub fn render_sidebar(app: &mut ManagerApp, cx: &mut Context<ManagerApp>) -> impl IntoElement {
    let mut nav = div().flex().flex_col().gap_0p5().p_2();
    for view in views::ALL {
        let active = app.view == *view;
        let weak = cx.weak_entity();
        nav = nav.child(
            div()
                .id(SharedString::from(format!("nav-{:?}", view)))
                .h_8()
                .w_full()
                .flex()
                .items_center()
                .gap_2()
                .px_2p5()
                .rounded_md()
                .cursor_pointer()
                .when(active, |d| d.bg(fade(FG, 0.12)))
                .when(!active, |d| d.hover(|s| s.bg(fade(FG, 0.07))))
                .child(
                    gpui_component::Icon::new(view.icon())
                        .with_size(px(16.))
                        .text_color(if active { c(FG) } else { c(MUTED_FG) }),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_size(px(13.))
                        .text_color(if active { c(FG) } else { c(MUTED_FG) })
                        .child(view.label()),
                )
                .on_click(move |_, _, cx| {
                    weak.update(cx, |this, cx| this.set_view(*view, cx)).ok();
                }),
        );
    }
    div()
        .w(px(SIDEBAR_W))
        .h_full()
        .flex_none()
        .bg(c(SIDEBAR_BG))
        .border_r_1()
        .border_color(c(SIDEBAR_DIVIDER))
        .flex()
        .flex_col()
        .child(
            div()
                .h(px(TITLEBAR_H))
                .px_4()
                .flex()
                .items_center()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .child("Kosmos Manager")
                .window_control_area(WindowControlArea::Drag),
        )
        .child(div().flex_1().overflow_y_scrollbar().child(nav))
        .child(
            div()
                .p_3()
                .border_t_1()
                .border_color(c(SIDEBAR_DIVIDER))
                .text_xs()
                .text_color(c(MUTED_FG))
                .child(format!("GPUI v{}", env!("CARGO_PKG_VERSION"))),
        )
}

pub fn render_titlebar(app: &ManagerApp, cx: &mut Context<ManagerApp>) -> impl IntoElement {
    let meta = views::meta(app.view);
    div()
        .h(px(TITLEBAR_H))
        .flex_none()
        .flex()
        .items_center()
        .px_4()
        .border_b_1()
        .border_color(c(BORDER))
        .window_control_area(WindowControlArea::Drag)
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(meta.0),
                )
                .child(div().text_xs().text_color(c(MUTED_FG)).child(meta.1)),
        )
        .child(
            Button::new("refresh")
                .ghost()
                .label("Обновить")
                .on_click(cx.listener(|this, _, _, cx| {
                    this.load_current();
                    cx.notify();
                })),
        )
}

pub fn render_banner(app: &ManagerApp, cx: &mut Context<ManagerApp>) -> impl IntoElement {
    let text = if app.error.is_some() {
        app.error.clone().unwrap_or_default()
    } else {
        "Запрос выполняется…".into()
    };
    div()
        .absolute()
        .bottom_3()
        .left(px(SIDEBAR_W + 12.0))
        .right_3()
        .p_3()
        .rounded_md()
        .border_1()
        .border_color(c(BORDER))
        .bg(c(POPOVER))
        .flex()
        .items_center()
        .gap_3()
        .child(div().flex_1().text_sm().child(text))
        .when(app.error.is_some(), |d| {
            d.child(Button::new("retry").label("Обновить").on_click(cx.listener(
                |this, _, _, cx| {
                    this.error = None;
                    this.load_current();
                    cx.notify();
                },
            )))
        })
}
