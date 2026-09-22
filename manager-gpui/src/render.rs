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
        self.sync_theme(window, cx);
        let sidebar_p = self.sidebar_progress(window);
        let active = views::render(self.view, self, window, cx);
        let mut root = div()
            .size_full()
            .flex()
            .relative()
            .overflow_hidden()
            .bg(c(BG()))
            .text_color(c(FG()))
            .font_family("Inter")
            .text_size(px(13.))
            .line_height(px(20.))
            .child(render_sidebar(self, sidebar_p, cx))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .child(render_titlebar(self, sidebar_p, cx))
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
            )
            .child(render_sidebar_toggle(cx));
        if self.dev_fps {
            root = root.child(
                div()
                    .absolute()
                    .bottom_3()
                    .right_3()
                    .child(self.fps_view.clone()),
            );
        }
        if self.error.is_some() || self.action_busy {
            root = root.child(render_banner(self, sidebar_p, cx));
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
