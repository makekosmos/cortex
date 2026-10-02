//! Данные — manager.data.summary/types/list/search/storage. Type picker
//! left, objects right; search hits the Engine search op (DataView.vue
//! parity), storage breakdown comes from `manager.data.storage`.
use ::gpui::{prelude::*, *};
use gpui_component::input::Input;
use serde_json::json;

use crate::app::ManagerApp;
use crate::async_fields::field_text;
use crate::widgets::*;
use mundus_gpui_kit::theme::*;

pub fn load(app: &mut ManagerApp) {
    app.call("data.summary", "manager.data.summary", json!({}));
    app.call("data.types", "manager.data.types", json!({}));
    app.call("data.storage", "manager.data.storage", json!({}));
    let type_id = app.data_type.clone();
    if let Some(t) = type_id {
        app.call(
            "data.list",
            "manager.data.list",
            json!({"type_id": t, "limit": 200}),
        );
    }
}

pub fn render(
    app: &mut ManagerApp,
    window: &mut Window,
    cx: &mut Context<ManagerApp>,
) -> AnyElement {
    let mut col = div().flex().flex_col().gap_4().w_full();
    col = col.child(slot_or(app, "data.summary", |v| {
        let mut cards = div().flex().gap_3().flex_wrap();
        for t in varr(v, "types") {
            cards = cards.child(
                card()
                    .w(px(200.))
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(c(MUTED_FG()))
                            .child(vstr(t, "name")),
                    )
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(vstr(t, "count")),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(c(MUTED_FG()))
                            .child(fmt_bytes(vnum(t, "logical_bytes"))),
                    ),
            );
        }
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(cards)
            .into_any_element()
    }));

    let total = field_text(app.slots.get("data.storage"), |v| {
        fmt_bytes(vnum(v, "total_bytes"))
    });
    col = col.child(
        card()
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .gap_2()
                    .child(div().text_size(px(13.)).child("Хранилище на диске"))
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(c(MUTED_FG()))
                            .child(format!("всего {total}")),
                    ),
            )
            .child(slot_or(app, "data.storage", |v| {
                let mut list = div().flex().flex_col().gap_2();
                for cat in varr(v, "categories") {
                    let mut cat_row = row(vstr(cat, "label"), fmt_bytes(vnum(cat, "bytes")));
                    if vstr(cat, "id") == "legacy_quarantine" {
                        cat_row = cat_row.child(btn_id("quarantine-clear", "Очистить", {
                            cx.listener(|this, _, _, cx| {
                                this.ask_confirm(
                                    "Удалить устаревшие данные?",
                                    "Это файлы, оставшиеся от старой версии приложения. \
                             Они не нужны для работы, но удаление необратимо.",
                                    "manager.data.quarantine.clear",
                                    json!({}),
                                    cx,
                                );
                            })
                        }));
                    }
                    list = list.child(cat_row);
                    for part in varr(cat, "detail") {
                        list = list.child(
                            div()
                                .pl_4()
                                .child(row(vstr(part, "label"), fmt_bytes(vnum(part, "bytes")))),
                        );
                    }
                }
                list.into_any_element()
            })),
    );

    let search = app.input("data.search", "Поиск объектов…", false, window, cx);
    col = col.child(
        div()
            .flex()
            .gap_2()
            .items_center()
            .child(div().w(px(320.)).child(Input::new(&search)))
            .child(btn("data-search", "Найти", false, cx, |this, cx| {
                let q = this.input_value("data.search", cx);
                if !q.is_empty() {
                    this.call("data.search", "manager.data.search", json!({"query": q}));
                }
            })),
    );

    let mut body = div().flex().gap_4().w_full().items_start();
    let mut types = card().w(px(260.)).flex_none();
    types = types.child(
        div()
            .text_size(px(12.))
            .text_color(c(MUTED_FG()))
            .child("Типы объектов"),
    );
    let types_val = app.data("data.types");
    let types_list = if types_val.is_array() {
        types_val.as_array().cloned().unwrap_or_default()
    } else {
        varr(&types_val, "items").to_vec()
    };
    for t in types_list {
        let id = vstr(&t, "type_id");
        let name = vstr(&t, "name");
        let count = vnum(&t, "count");
        let selected = app.data_type.as_deref() == Some(id.as_str());
        // A type without a display name still must not render its id
        // (`coding_profile_obj` & co. are internal keys — KOS-279).
        let row_name = if name.is_empty() {
            "Тип данных".to_string()
        } else {
            name.clone()
        };
        types = types.child(
            div()
                .id(SharedString::from(format!("type-{id}")))
                .h_8()
                .px_2()
                .rounded_md()
                .flex()
                .items_center()
                .gap_2()
                .cursor_pointer()
                .when(selected, |d| d.bg(fade(ACCENT(), 0.18)))
                .when(!selected, |d| d.hover(|s| s.bg(fade(FG(), 0.06))))
                .child(div().flex_1().text_size(px(13.)).child(row_name.clone()))
                .child(badge(format!("{count:.0}"), MUTED_FG()))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.data_type = Some(id.clone());
                    this.call(
                        "data.list",
                        "manager.data.list",
                        json!({"type_id": id, "limit": 200}),
                    );
                    cx.notify();
                }))
                .role(Role::Button)
                .aria_label(row_name)
                .aria_selected(selected),
        );
    }
    body = body.child(types);

    let mut right = div().flex_1().flex().flex_col().gap_3();
    let results = app.data("data.search");
    let items = if !results.is_null() {
        varr(&results, "items").to_vec()
    } else {
        varr(&app.data("data.list"), "items").to_vec()
    };
    let header = if !results.is_null() {
        format!("Результаты поиска ({})", items.len())
    } else {
        format!("Объекты ({})", items.len())
    };
    let mut list = card();
    list = list.child(
        div()
            .text_size(px(12.))
            .text_color(c(MUTED_FG()))
            .child(header),
    );
    if items.is_empty() {
        list = list.child(empty("Нет объектов"));
    }
    for item in items.iter().take(200) {
        let title = {
            let t = vopt(item, "title").or_else(|| vopt(item, "name"));
            // An opaque object id is not a title (KOS-279).
            t.filter(|s| !s.is_empty())
                .unwrap_or_else(|| "Без названия".into())
        };
        let sub = vopt(item, "updated_at")
            .or_else(|| vopt(item, "modified_at"))
            .unwrap_or_default();
        list = list.child(row(title, sub));
    }
    right = right.child(list);
    body = body.child(right);
    col.child(body).into_any_element()
}
