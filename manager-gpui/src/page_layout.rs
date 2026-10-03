//! Physical layout grid, semantic spacing and independently scaled typography.
use crate::theme::*;
use ::gpui::{prelude::*, *};

pub const PAGE_WIDTH: f32 = 760.;
pub const INSET: f32 = 16.;
pub const GAP: f32 = 12.;
pub const SECTION_GAP: f32 = 32.;
pub const LABEL_WIDTH: f32 = 180.;

#[cfg(test)]
#[path = "page_layout_tests.rs"]
mod tests;

pub fn input_field(
    state: &Entity<gpui_component::input::InputState>,
) -> gpui_component::input::Input {
    gpui_component::input::Input::new(state)
        .h(px(32.))
        .text_size(ui_px(13.))
        .line_height(ui_px(18.))
}

pub fn empty(text: &str) -> Div {
    div()
        .w_full()
        .min_w_0()
        .py(px(12.))
        .text_size(ui_px(13.))
        .line_height(ui_px(18.))
        .text_color(c(MUTED_FG()))
        .child(text.to_owned())
}

/// Ordinary adjacent cards; semantic groups use page_sections instead.
pub fn page_stack() -> Div {
    div().w_full().min_w_0().flex().flex_col().gap(px(24.))
}

/// More space before a new subject, not between its heading and controls.
pub fn page_sections() -> Div {
    div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(SECTION_GAP))
}

pub fn section_group() -> Div {
    div().w_full().min_w_0().flex().flex_col().gap(px(GAP))
}

pub fn section(title: &str, hint: &str) -> Div {
    div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(
            div()
                .text_size(ui_px(15.))
                .line_height(ui_px(20.))
                .font_weight(FontWeight::SEMIBOLD)
                .child(title.to_owned()),
        )
        .when(!hint.is_empty(), |d| {
            d.child(
                div()
                    .text_size(ui_px(12.))
                    .line_height(ui_px(16.))
                    .text_color(c(MUTED_FG()))
                    .child(hint.to_owned()),
            )
        })
}

pub fn card() -> Div {
    div()
        .w_full()
        .min_w_0()
        .rounded(px(12.))
        .border_1()
        .border_color(c(BORDER()))
        .bg(c(CARD()))
        .px(px(INSET))
        .py(px(12.))
        .flex()
        .flex_col()
        .gap(px(GAP))
}

pub fn row_copy(label: impl Into<String>, sub: impl Into<String>) -> Div {
    let sub = sub.into();
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(2.))
        .child(
            div()
                .text_size(ui_px(13.))
                .line_height(ui_px(18.))
                .font_weight(FontWeight::MEDIUM)
                .whitespace_nowrap()
                .overflow_hidden()
                .text_ellipsis()
                .child(label.into()),
        )
        .when(!sub.is_empty(), |d| {
            d.child(
                div()
                    .text_size(ui_px(12.))
                    .line_height(ui_px(16.))
                    .text_color(c(MUTED_FG()))
                    .child(sub),
            )
        })
}

pub fn row(label: impl Into<String>, sub: impl Into<String>) -> Div {
    div()
        .w_full()
        .min_w_0()
        .min_h(px(36.))
        .flex()
        .items_center()
        .gap(px(GAP))
        .child(row_copy(label, sub))
}

fn key_column(key: &str) -> Div {
    div()
        .w(px(LABEL_WIDTH))
        .flex_none()
        .text_size(ui_px(13.))
        .line_height(ui_px(18.))
        .text_color(c(MUTED_FG()))
        .child(key.to_owned())
}

pub fn kv(key: &str, value: impl Into<String>) -> Div {
    div()
        .w_full()
        .min_w_0()
        .flex()
        .items_start()
        .gap(px(GAP))
        .child(key_column(key))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(ui_px(13.))
                .line_height(ui_px(18.))
                .child(value.into()),
        )
}
