//! Collapsible disk usage, with a single left-aligned size column.
use crate::{app::ManagerApp, async_fields::field_text, theme::*, widgets::*};
use ::gpui::{prelude::*, *};
use serde_json::json;

#[cfg(test)]
#[path = "data_storage_tests.rs"]
mod tests;

fn storage_row(label: String, bytes: String, detail: bool) -> Div {
    let selector = format!("storage-size-{label}");
    div()
        .w_full()
        .flex()
        .items_center()
        .gap(px(12.))
        .text_size(ui_px(13.))
        .line_height(ui_px(18.))
        .child(
            div()
                .w(relative(0.55))
                .flex_none()
                .min_w_0()
                .when(detail, |el| el.pl(px(16.)).text_color(c(MUTED_FG())))
                .child(label),
        )
        .child(
            div()
                .debug_selector(move || selector.clone())
                .flex_1()
                .min_w(px(72.))
                .text_color(c(MUTED_FG()))
                .child(bytes),
        )
}

pub fn render(app: &ManagerApp, cx: &mut Context<ManagerApp>) -> AnyElement {
    let total = field_text(app.slots.get("data.storage"), |v| {
        fmt_bytes(vnum(v, "total_bytes"))
    });
    let mut body = card()
        .id("data-storage-card")
        .debug_selector(|| "data-storage-card".into())
        .px(px(0.))
        .py(px(0.))
        .gap(px(0.))
        .child(
            div()
                .id("data-storage-toggle")
                .debug_selector(|| "data-storage-toggle".into())
                .w_full()
                .rounded_t(px(12.))
                .when(!app.data_storage_open, |el| el.rounded_b(px(12.)))
                .flex()
                .items_center()
                .gap_3()
                .px(px(crate::page_layout::INSET))
                .py(px(10.))
                .child(
                    storage_row("Хранилище на диске".into(), total, false)
                        .flex_1()
                        .min_w_0(),
                )
                .child(
                    gpui_component::Icon::default()
                        .path(if app.data_storage_open {
                            "icons/chevron-up.svg"
                        } else {
                            "icons/chevron-down.svg"
                        })
                        .size(px(16.))
                        .text_color(c(MUTED_FG())),
                )
                .cursor_pointer()
                .hover(|style| style.bg(fade(FG(), 0.05)))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.data_storage_open = !this.data_storage_open;
                    cx.notify();
                }))
                .role(Role::Button)
                .aria_label("Разбивка хранилища на диске")
                .aria_expanded(app.data_storage_open),
        );
    if app.data_storage_open {
        body = body.child(
            div()
                .id("data-storage-list")
                .debug_selector(|| "data-storage-list".into())
                .w_full()
                .border_t_1()
                .border_color(fade(BORDER(), 0.6))
                // Reserve the header chevron's width in every row so total,
                // category and detail values all begin at the same x coordinate.
                .child(slot_or(app, "data.storage", |v| {
                    let mut list = div().w_full().flex().flex_col();
                    for (index, cat) in varr(v, "categories").iter().enumerate() {
                        let mut group = div()
                            .w_full()
                            .flex()
                            .flex_col()
                            .px(px(crate::page_layout::INSET))
                            .py(px(8.))
                            .when(index > 0, |el| {
                                el.border_t_1().border_color(fade(BORDER(), 0.6))
                            });
                        let mut category =
                            storage_row(vstr(cat, "label"), fmt_bytes(vnum(cat, "bytes")), false);
                        if vstr(cat, "id") == "legacy_quarantine" {
                            category = category.child(btn_id(
                                "quarantine-clear",
                                "Очистить",
                                cx.listener(|this, _, _, cx| {
                                    this.ask_confirm(
                                        "Удалить устаревшие данные?",
                                        concat!(
                                            "Это файлы, оставшиеся от старой версии приложения. ",
                                            "Они не нужны для работы, но удаление необратимо."
                                        ),
                                        "manager.data.quarantine.clear",
                                        json!({}),
                                        cx,
                                    );
                                }),
                            ));
                        }
                        group = group.child(div().pr(px(28.)).child(category));
                        for part in varr(cat, "detail") {
                            group = group.child(div().pt(px(8.)).pr(px(28.)).child(storage_row(
                                vstr(part, "label"),
                                fmt_bytes(vnum(part, "bytes")),
                                true,
                            )));
                        }
                        list = list.child(group);
                    }
                    list.into_any_element()
                })),
        );
    }
    body.into_any_element()
}
