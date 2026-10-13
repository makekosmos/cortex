//! Storage overview with linked segments and measured disclosure animation.
use super::storage_motion::StorageUi;
#[path = "storage_snapshot.rs"]
mod snapshot;
use crate::{app::ManagerApp, theme::*, widgets::*};
use gpui::{prelude::*, *};
use gpui_component::{collapsible::Collapsible, tooltip::Tooltip};
use serde_json::{json, Value};
pub(crate) use snapshot::normalize;
use snapshot::{segments, selected_categories, sort_items};

pub fn load(app: &mut ManagerApp) {
    app.call("data.storage", "manager.data.storage", json!({}));
}

fn color(id: &str) -> u32 {
    match id {
        "database" | "objects" => 0xf09c0e,
        "backups" | "sqlite_reserved" | "free_pages" | "file_reserve" => 0xd9b852,
        "dictation_models" | "types" => 0x9b87db,
        "packages" | "sync" => 0x659bc8,
        "updates" => 0x80ab73,
        "file_index" | "search_index" => 0x65b9b0,
        "app_index" | "migrations" => 0xb79a7e,
        "installations" | "links" => 0xce8293,
        "cache" | "local_state" => 0x88acbe,
        "logs" => 0xa7a7a7,
        "settings" | "integrations" => 0xab9cb8,
        "runtime_state" | "schema" => 0x8f9b7a,
        "recordings" | "usage_tracker" => 0xd79067,
        "tools" | "-shm" => 0x729f96,
        "-wal" | "-journal" => 0x80ab73,
        _ => 0x89919b,
    }
}

fn detail_row(part: &Value) -> Div {
    div()
        .w_full()
        .flex()
        .items_center()
        .gap_3()
        .py(px(8.))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .whitespace_nowrap()
                .overflow_hidden()
                .text_ellipsis()
                .text_color(c(MUTED_FG()))
                .child(vstr(part, "label")),
        )
        .child(
            div()
                .flex_none()
                .text_color(c(MUTED_FG()))
                .font_family("Geist Mono")
                .child(fmt_bytes(vnum(part, "bytes"))),
        )
}

fn category(
    app: &ManagerApp,
    part: &Value,
    last: bool,
    cx: &mut Context<ManagerApp>,
) -> AnyElement {
    let id = vstr(part, "id");
    let selector = format!("storage-category-{id}");
    let expandable = !varr(part, "detail").is_empty();
    let open = expandable && app.storage_ui.open.contains(&id);
    let toggle_id = id.clone();
    let mut details = div()
        .w_full()
        .pl(px(42.))
        .pr(px(16.))
        .pb(px(8.))
        .flex()
        .flex_col();
    let mut sorted_details = varr(part, "detail").to_vec();
    sort_items(&mut sorted_details);
    for detail in &sorted_details {
        details = details.child(detail_row(detail));
    }
    div()
        .id(SharedString::from(selector.clone()))
        .debug_selector(move || selector.clone())
        .anchor_scroll(Some(app.storage_ui.anchor(&id)))
        .w_full()
        .min_w_0()
        .child(
            Collapsible::new()
                .motion_id(SharedString::from(format!("storage-reveal-{id}")))
                .open(open)
                .child(
                    div()
                        .id(SharedString::from(format!("storage-toggle-{id}")))
                        .debug_selector(move || format!("storage-toggle-{id}"))
                        .w_full()
                        .px(px(16.))
                        .py(px(11.))
                        .rounded(px(0.))
                        .when(last && !open, |row| row.rounded_b(px(12.)))
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .when(expandable, |row| {
                            row.cursor_pointer()
                                .hover(|style| style.bg(fade(FG(), 0.05)))
                        })
                        .child(
                            div()
                                .size(px(16.))
                                .rounded_full()
                                .bg(c(color(&vstr(part, "id")))),
                        )
                        .child(div().flex_1().min_w_0().child(vstr(part, "label")))
                        .child(
                            div()
                                .flex_none()
                                .text_color(c(MUTED_FG()))
                                .font_family("Geist Mono")
                                .child(fmt_bytes(vnum(part, "bytes"))),
                        )
                        .when(expandable, |row| {
                            row.on_click(cx.listener(move |app, _, _, cx| {
                                if !app.storage_ui.open.remove(&toggle_id) {
                                    app.storage_ui.open.insert(toggle_id.clone());
                                }
                                cx.notify();
                            }))
                            .role(Role::Button)
                            .aria_label(vstr(part, "label"))
                            .aria_expanded(open)
                        }),
                )
                .content(details),
        )
        .into_any_element()
}

pub fn render(app: &ManagerApp, cx: &mut Context<ManagerApp>) -> AnyElement {
    let database_only = app.storage_database_only;
    page_stack()
        .child(
            div().flex().items_center().gap_2().child(
                gpui_component::tab::TabBar::new("storage-scope")
                    .segmented()
                    .children([
                        gpui_component::tab::Tab::new()
                            .label("Все")
                            .debug_selector(|| "storage-all".into()),
                        gpui_component::tab::Tab::new()
                            .label("БД")
                            .debug_selector(|| "storage-database".into()),
                    ])
                    .selected_index(usize::from(database_only))
                    .on_click(cx.listener(|app, index: &usize, _, cx| {
                        app.storage_database_only = *index == 1;
                        app.storage_ui.scroll.set_offset(point(px(0.), px(0.)));
                        cx.notify();
                    })),
            ),
        )
        .child(slot_or(app, "data.storage", |value| {
            let categories = selected_categories(value, database_only);
            let total: f64 = categories.iter().map(|part| vnum(part, "bytes")).sum();
            let parts = segments(value, database_only);
            let segment_total: f64 = parts.iter().map(|part| vnum(part, "bytes")).sum();
            let mut bar = div()
                .debug_selector(|| "storage-bar".into())
                .w_full()
                .h(px(27.))
                .flex()
                .gap(px(2.))
                .overflow_hidden();
            for part in &parts {
                let bytes = vnum(part, "bytes");
                if bytes <= 0. {
                    continue;
                }
                let id = vstr(part, "id");
                let segment_id = id.clone();
                let label = vstr(part, "label");
                let tip = format!("{label}: {}", fmt_bytes(bytes));
                let hover_id = segment_id.clone();
                let dimmed = app
                    .storage_ui
                    .hovered
                    .as_deref()
                    .is_some_and(|hovered| hovered != segment_id);
                bar = bar.child(
                    div()
                        .id(SharedString::from(format!("storage-segment-{id}")))
                        .debug_selector(move || format!("storage-segment-{id}"))
                        .h_full()
                        .flex_1()
                        .flex_grow((100. * bytes / segment_total.max(1.)) as f32)
                        .min_w(px(4.))
                        .rounded(px(5.))
                        .bg({
                            let mut fill = c(color(&vstr(part, "id")));
                            if dimmed {
                                fill.s *= 0.18;
                            }
                            fill
                        })
                        .cursor_pointer()
                        .on_hover(cx.listener(move |app, hovered: &bool, _, cx| {
                            if *hovered {
                                app.storage_ui.hovered = Some(hover_id.clone());
                            } else if app.storage_ui.hovered.as_deref() == Some(hover_id.as_str()) {
                                app.storage_ui.hovered = None;
                            }
                            cx.notify();
                        }))
                        .tooltip(move |window, cx| Tooltip::new(tip.clone()).build(window, cx))
                        .on_click(cx.listener(move |app, _, window, cx| {
                            StorageUi::reveal(app, segment_id.clone(), window, cx);
                        }))
                        .role(Role::Button)
                        .aria_label(format!("Показать: {label}")),
                );
            }
            let mut body = card()
                .px(px(0.))
                .pt(px(24.))
                .pb(px(0.))
                .gap(px(16.))
                .child(
                    div()
                        .debug_selector(|| "storage-total".into())
                        .px(px(16.))
                        .text_size(ui_px(28.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(fmt_bytes(total)),
                )
                .child(
                    div()
                        .px(px(16.))
                        .text_size(ui_px(12.))
                        .text_color(c(MUTED_FG()))
                        .child(if database_only {
                            "Содержимое рабочей базы ARK и журнал SQLite"
                        } else {
                            "Установки, данные и локальные ИИ-модели Mundus"
                        }),
                )
                .child(div().px(px(16.)).mb(px(8.)).child(bar));
            let mut list = div().w_full().flex().flex_col().gap(px(0.));
            for (index, part) in parts.iter().enumerate() {
                list = list.child(category(app, part, index + 1 == parts.len(), cx));
            }
            body = body.child(list);
            body.into_any_element()
        }))
        .into_any_element()
}

#[cfg(test)]
#[path = "storage_tests.rs"]
mod tests;
