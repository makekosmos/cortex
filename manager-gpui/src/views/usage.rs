//! Затреканное время — KOS-287: список сортируется по «активному» времени
//! (foreground && !idle, считает engine), группируется по exe-идентичности
//! (`get_usage_analytics` merge'ит legacy строки) и рендерится через
//! `v_virtual_list` — раньше 500 строк собирались в DOM целиком и скролл
//! подвисал. gpui-fast не используем: он сделан под zed-gpui, а в workspace
//! уже есть `gpui_component::v_virtual_list` с той же семантикой.
//!
//! Sort/filter state + cell formatting live in `usage/rows.rs`.
use std::rc::Rc;

use ::gpui::{prelude::*, *};
use gpui_component::v_virtual_list;

use crate::app::ManagerApp;
use crate::theme::*;
use crate::widgets::*;

mod rows;
pub use rows::{build_usage_rows, UsageColumn, UsageRow, UsageSort};

/// Fixed row height — v_virtual_list needs `item_sizes` upfront; uniform rows
/// keep the arithmetic exact instead of measuring every row.
const ROW_H: f32 = 40.;

pub fn load(app: &mut ManagerApp) {
    app.usage_report("usage.report");
}

pub fn render(
    app: &mut ManagerApp,
    _window: &mut Window,
    cx: &mut Context<ManagerApp>,
) -> AnyElement {
    let sort = app.usage_sort;
    let show_system = app.usage_show_system;
    let rows = app.usage_rows.clone();
    let scroll = app.usage_scroll.clone();
    let mut col = div().flex().flex_col().gap_4().w_full().h_full();
    col = col.child(
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(
                toggle("usage-show-system", show_system, cx, |this, checked, _| {
                    this.set_usage_show_system(checked);
                })
                .accessibility_label("Показывать системные процессы"),
            )
            .child(
                div()
                    .text_size(crate::theme::ui_px(12.))
                    .text_color(c(MUTED_FG()))
                    .child("Показывать системные процессы"),
            ),
    );
    col = col.child(slot_or(app, "usage.report", |_v| {
        usage_table(rows, sort, scroll, cx)
    }));
    col.into_any_element()
}

fn usage_table(
    rows: Rc<Vec<UsageRow>>,
    sort: UsageSort,
    scroll: gpui_component::VirtualListScrollHandle,
    cx: &mut Context<ManagerApp>,
) -> AnyElement {
    let mut table = card().flex().flex_col().flex_1().min_h_0();
    table = table.child(header_row(sort, cx));
    if rows.is_empty() {
        table = table.child(empty("Затреканное время не найдено"));
    } else {
        let sizes = Rc::new(vec![size(px(0.), px(ROW_H)); rows.len()]);
        let list = v_virtual_list(
            cx.entity(),
            "usage-rows",
            sizes,
            |app, range, _window, _cx| {
                // Pure indexing into rows prepared on data/sort/filter change —
                // cloning the report and re-sorting per visible-range render
                // was the per-frame O(n log n) stutter.
                range
                    .filter_map(|ix| app.usage_rows.get(ix).map(|row| usage_row(ix, row)))
                    .collect()
            },
        )
        .track_scroll(&scroll);
        table = table.child(list.flex_1().min_h_0().w_full());
    }
    table.into_any_element()
}

fn sort_arrow(sort: UsageSort, column: UsageColumn) -> &'static str {
    if sort.column != column {
        return "";
    }
    if sort.ascending {
        " ↑"
    } else {
        " ↓"
    }
}

fn head_cell(
    text: &'static str,
    width: f32,
    column: UsageColumn,
    sort: UsageSort,
    cx: &mut Context<ManagerApp>,
) -> Stateful<Div> {
    div()
        .id(SharedString::from(format!("usage-sort-{text}")))
        .w(px(width))
        .flex_none()
        .text_size(crate::theme::ui_px(11.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(c(MUTED_FG()))
        .whitespace_nowrap()
        .overflow_hidden()
        .cursor_pointer()
        .child(format!("{}{}", text, sort_arrow(sort, column)))
        .role(Role::Button)
        .aria_label(text)
        .on_click(cx.listener(move |this, _, _, cx| {
            this.toggle_usage_sort(column);
            cx.notify();
        }))
}

fn head_grow(
    text: &'static str,
    column: UsageColumn,
    sort: UsageSort,
    cx: &mut Context<ManagerApp>,
) -> Stateful<Div> {
    div()
        .id(SharedString::from(format!("usage-sort-{text}")))
        .flex_1()
        .min_w_0()
        .text_size(crate::theme::ui_px(11.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(c(MUTED_FG()))
        .whitespace_nowrap()
        .overflow_hidden()
        .cursor_pointer()
        .child(format!("{}{}", text, sort_arrow(sort, column)))
        .role(Role::Button)
        .aria_label(text)
        .on_click(cx.listener(move |this, _, _, cx| {
            this.toggle_usage_sort(column);
            cx.notify();
        }))
}

fn header_row(sort: UsageSort, cx: &mut Context<ManagerApp>) -> Div {
    div()
        .w_full()
        .flex()
        .items_center()
        .gap_3()
        .pb_2()
        .border_b_1()
        .border_color(c(BORDER()))
        .child(div().w(px(32.)).flex_none())
        .child(head_grow("Приложение", UsageColumn::Name, sort, cx))
        .child(head_cell("Активно", 110., UsageColumn::Active, sort, cx))
        .child(head_cell("Запусков", 76., UsageColumn::Sessions, sort, cx))
        .child(head_cell(
            "Последний запуск",
            150.,
            UsageColumn::LastSeen,
            sort,
            cx,
        ))
        .child(head_grow("Путь", UsageColumn::Path, sort, cx))
}

fn metric(text: String, width: f32) -> Div {
    div()
        .w(px(width))
        .flex_none()
        .text_size(crate::theme::ui_px(13.))
        .whitespace_nowrap()
        .overflow_hidden()
        .child(text)
}

fn usage_row(ix: usize, entry: &UsageRow) -> Stateful<Div> {
    // Shared 32px icon slot: cached PNG via app_index.icon_path / exe_info, letter
    // badge underneath when the cache has nothing or the file fails to load.
    let icon = app_icon(icon_file(entry.icon_path.clone()), &entry.name);

    div()
        .id(ElementId::NamedInteger("usage-row".into(), ix as u64))
        .w_full()
        .h(px(ROW_H))
        .flex()
        .items_center()
        .gap_3()
        .role(Role::ListItem)
        .aria_label(entry.name.clone())
        .child(icon)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(crate::theme::ui_px(13.))
                .font_weight(FontWeight::MEDIUM)
                .whitespace_nowrap()
                .text_ellipsis()
                .overflow_hidden()
                .child(entry.name.clone()),
        )
        .child(metric(entry.active.clone(), 110.))
        .child(metric(entry.sessions.clone(), 76.))
        .child(metric(entry.last_seen.clone(), 150.))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(crate::theme::ui_px(12.))
                .text_color(fade(FG(), 0.65))
                .whitespace_nowrap()
                .text_ellipsis()
                .overflow_hidden()
                .child(entry.path.clone()),
        )
}
