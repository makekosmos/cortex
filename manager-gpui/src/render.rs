//! Root layout: sidebar + titlebar + active view, with the Engine banner,
//! confirm modal and disclosure/detail overlays layered on top.
use ::gpui::{prelude::*, *};
use gpui_component::scroll::ScrollableElement;

use crate::app::ManagerApp;
use crate::modals::{render_confirm, render_overlay};
use crate::theme::*;
use crate::views;
use crate::widgets::*;

const PAGE_MAX_WIDTH: f32 = crate::page_layout::PAGE_WIDTH;

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;

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
            .bg(shell_fill())
            .text_color(c(FG()))
            .font_family(self.appearance.resolve(window).font_family)
            .text_size(ui_px(13.))
            .line_height(ui_px(20.))
            .child(render_sidebar(self, sidebar_p, window, cx))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .bg(panel_fill())
                    .child(render_titlebar(sidebar_p, window))
                    .child(
                        div()
                            .id("page-content")
                            .debug_selector(|| "page-content".into())
                            .flex_1()
                            .overflow_y_scrollbar()
                            .p_6()
                            .flex()
                            .flex_col()
                            .gap_4()
                            .child(
                                div()
                                    .id("page-column")
                                    .debug_selector(|| "page-column".into())
                                    .w_full()
                                    .max_w(px(PAGE_MAX_WIDTH))
                                    .mx_auto()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .when(self.view == views::View::Usage, |column| {
                                        column.flex_1().min_h_0()
                                    })
                                    .when(self.view != views::View::Usage, |column| {
                                        column.flex_none()
                                    })
                                    .child(active),
                            ),
                    ),
            )
            .child(render_sidebar_toggle(self.sidebar_target > 0.5, window, cx));
        if self.dev_fps {
            root = root.child(
                div()
                    .absolute()
                    .bottom_3()
                    .right_3()
                    .child(self.fps_view.clone()),
            );
        }
        if self.error.is_some() {
            root = root.child(render_banner(self, sidebar_p, cx));
        }
        if let Some(confirm) = &self.confirm {
            root = root.child(render_confirm(confirm, cx));
        }
        if self.key_editor.is_some() {
            root = root.child(views::secrets::render_modal(self, window, cx));
        }
        if self.disclosure.is_some() || self.detail.is_some() {
            root = root.child(render_overlay(self, cx));
        }
        root
    }
}
