//! Root layout: sidebar + titlebar + active view, with the Engine banner,
//! confirm modal and disclosure/detail overlays layered on top.
use ::gpui::{prelude::*, *};
use gpui_component::scroll::ScrollableElement;

use crate::app::ManagerApp;
use crate::modals::{render_confirm, render_overlay};
use crate::theme::*;
use crate::views;
use crate::widgets::*;

impl Render for ManagerApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let active = views::render(self.view, self, window, cx);
        let mut root = div()
            .size_full()
            .flex()
            .bg(c(BG))
            .text_color(c(FG))
            .child(render_sidebar(self, cx))
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .child(render_titlebar(self, cx))
                    .child(
                        div()
                            .flex_1()
                            .overflow_y_scrollbar()
                            .p_6()
                            .flex()
                            .flex_col()
                            .gap_4()
                            .child(active),
                    ),
            );
        if self.error.is_some() || self.action_busy {
            root = root.child(render_banner(self, cx));
        }
        if let Some(confirm) = &self.confirm {
            root = root.child(render_confirm(confirm, cx));
        }
        if self.disclosure.is_some() || self.detail.is_some() {
            root = root.child(render_overlay(self, cx));
        }
        root
    }
}
