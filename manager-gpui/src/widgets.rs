//! Shared chrome: sidebar, titlebar, Engine banner. Modals live in
//! `modals.rs`, view primitives and JSON accessors in `fields.rs`
//! (re-exported here). Shell pieces come from `imago_gpui::chrome`.
use ::gpui::{prelude::*, *};
use imago_gpui::chrome::{self, SIDEBAR_W};

pub use crate::fields::*;

use crate::app::ManagerApp;
use crate::theme::*;
use crate::views;

// --- Chrome -----------------------------------------------------------------

pub fn render_sidebar(app: &mut ManagerApp, cx: &mut Context<ManagerApp>) -> impl IntoElement {
    let mut nav = chrome::sidebar_body();
    for view in views::ALL {
        let active = app.view == *view;
        let weak = cx.weak_entity();
        nav = nav.child(
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
    chrome::sidebar()
        .child(
            chrome::sidebar_titlebar()
                .px_4()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .child("Kosmos Manager")
                .window_control_area(WindowControlArea::Drag),
        )
        .child(nav)
        .child(
            chrome::sidebar_footer()
                .text_xs()
                .text_color(c(MUTED_FG))
                .child(format!("GPUI v{}", env!("CARGO_PKG_VERSION"))),
        )
}

pub fn render_titlebar(app: &ManagerApp, cx: &mut Context<ManagerApp>) -> impl IntoElement {
    let meta = views::meta(app.view);
    chrome::titlebar()
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
            imago_gpui::button::ghost("refresh")
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
