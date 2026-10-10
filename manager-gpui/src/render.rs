//! Root layout: sidebar + titlebar + active view, with the Engine banner,
//! confirm modal and disclosure/detail overlays layered on top.
use std::time::Instant;

use ::gpui::{prelude::*, *};
use gpui_component::scroll::ScrollableElement;
use serde_json::Value;

use crate::app::ManagerApp;
use crate::modals::{render_confirm, render_overlay, render_pairing_prompt};
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
                                    .gap(px(24.))
                                    .when(self.view != views::View::About, |column| {
                                        column.child(
                                            section(self.view.label(), "")
                                                .id("page-heading")
                                                .debug_selector(|| "page-heading".into()),
                                        )
                                    })
                                    .child(
                                        div()
                                            .id("page-body")
                                            .debug_selector(|| "page-body".into())
                                            .w_full()
                                            .min_w_0()
                                            .when(self.view == views::View::Usage, |body| {
                                                body.flex_1().min_h_0()
                                            })
                                            .child(active),
                                    ),
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
        if self.error.is_some() || self.notice.is_some() {
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
        // KOS-369: an unanswered pairing request pops on top of any view.
        let pairing_request = self
            .data("sync.snapshot")
            .get("incoming_pairing_requests")
            .and_then(Value::as_array)
            .and_then(|requests| requests.first().cloned());
        if let Some(request) = pairing_request {
            root = root.child(render_pairing_prompt(&request, cx));
        }
        // KOS-355: full-window update overlay, topmost layer. The state is
        // resolved every frame from the Engine status slot; `Failed` needs
        // the previous state, so the resolution is stored back on the app.
        let update_state = crate::update_overlay::resolve(
            self.update_snoozed,
            self.update_overlay.visible(),
            &vstr(&self.data("upd.mundus"), "state"),
        );
        if update_state.visible() && !self.update_overlay.visible() {
            self.update_anim_start = Instant::now();
        }
        self.update_overlay = update_state;
        if update_state.visible() {
            root = root.child(crate::update_overlay::render(
                self,
                update_state,
                window,
                cx,
            ));
        }
        root
    }
}
