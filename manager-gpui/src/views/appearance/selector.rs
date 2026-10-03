//! Settings selectors ported from Zeron's MIT-licensed settings widgets and
//! appearance page (Copyright 2026 Wing). The Engine still owns every value.
use super::*;
use crate::appearance_state::AppearanceMenu;
use gpui_component::scroll::ScrollableElement;
use std::time::{Duration, Instant};

#[derive(Clone)]
pub(super) struct OptionItem {
    pub label: String,
    pub value: String,
    pub palette: Option<(u32, u32, u32, u32)>,
}

impl OptionItem {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            palette: None,
        }
    }

    pub fn palette(mut self, surface: u32, background: u32, accent: u32, border: u32) -> Self {
        self.palette = Some((surface, background, accent, border));
        self
    }
}

fn chip((surface, background, accent, border): (u32, u32, u32, u32)) -> Div {
    div()
        .w(px(30.))
        .h(px(18.))
        .flex_none()
        .rounded(px(5.))
        .overflow_hidden()
        .border_1()
        .border_color(c(border))
        .flex()
        .child(div().w_1_3().h_full().bg(c(surface)))
        .child(div().w_1_3().h_full().bg(c(background)))
        .child(div().w_1_3().h_full().bg(c(accent)))
}

fn commit(
    app: &mut ManagerApp,
    kind: AppearanceMenu,
    option: &OptionItem,
    cx: &mut Context<ManagerApp>,
) {
    app.appearance.open_menu = None;
    app.appearance.font_menu_open = false;
    let params = match kind {
        AppearanceMenu::LightTheme => json!({"light_theme": option.value}),
        AppearanceMenu::DarkTheme => json!({"dark_theme": option.value}),
        AppearanceMenu::FontFamily => json!({"font_family": option.value}),
        AppearanceMenu::FontSize => {
            let Ok(size) = option.value.parse::<f32>() else {
                return;
            };
            json!({"font_size": size})
        }
    };
    patch(app, params, cx);
}

fn close(app: &mut ManagerApp, cx: &mut Context<ManagerApp>) {
    let dismissed = app.appearance.open_menu;
    app.appearance.open_menu = None;
    app.appearance.font_menu_open = false;
    app.appearance.menu_dismissed_at = dismissed.map(|kind| (kind, Instant::now()));
    cx.notify();
}

fn open(
    app: &mut ManagerApp,
    kind: AppearanceMenu,
    selected: usize,
    window: &mut Window,
    cx: &mut Context<ManagerApp>,
) {
    if !editable(app) {
        return;
    }
    app.appearance.open_menu = Some(kind);
    app.appearance.font_menu_open = kind == AppearanceMenu::FontFamily;
    app.appearance.menu_highlighted = selected;
    app.appearance.menu_query.clear();
    app.appearance.menu_generation += 1;
    app.appearance.menu_dismissed_at = None;
    if kind == AppearanceMenu::FontFamily {
        let input = watched_input(app, "appearance.font.search", "Поиск шрифта", window, cx);
        input.update(cx, |state, cx| state.set_value("", window, cx));
        window.focus(&input.read(cx).focus_handle(cx), cx);
    }
    cx.notify();
}

fn key_down(
    app: &mut ManagerApp,
    kind: AppearanceMenu,
    key: &str,
    selected: usize,
    options: &[OptionItem],
    window: &mut Window,
    cx: &mut Context<ManagerApp>,
) -> bool {
    let is_open = editable(app)
        && (app.appearance.open_menu == Some(kind)
            || (kind == AppearanceMenu::FontFamily && app.appearance.font_menu_open));
    if !is_open {
        if matches!(key, "up" | "down" | "enter" | "space") {
            open(app, kind, selected, window, cx);
            return true;
        }
        return false;
    }
    let count = options.len();
    match key {
        "up" => app.appearance.menu_highlighted = app.appearance.menu_highlighted.saturating_sub(1),
        "down" => {
            app.appearance.menu_highlighted =
                (app.appearance.menu_highlighted + 1).min(count.saturating_sub(1))
        }
        "home" => app.appearance.menu_highlighted = 0,
        "end" => app.appearance.menu_highlighted = count.saturating_sub(1),
        "escape" => {
            close(app, cx);
            return true;
        }
        "enter" | "space" if kind != AppearanceMenu::FontFamily || key == "enter" => {
            if let Some(option) = options.get(app.appearance.menu_highlighted) {
                commit(app, kind, option, cx);
            } else {
                close(app, cx);
            }
            return true;
        }
        _ => return false,
    }
    cx.notify();
    true
}

pub(super) struct SelectSpec {
    pub kind: AppearanceMenu,
    pub id: &'static str,
    pub aria_label: &'static str,
    pub current: String,
    pub options: Vec<OptionItem>,
    pub trigger_width: f32,
    pub menu_width: f32,
    pub heading: Option<&'static str>,
}

/// Source geometry: trigger h32/r8/pl10/pr8/gap8, 12.5px fixed type;
/// theme 218/260, font 220, size 128. The popover stays out of page flow.
pub(super) fn select(
    app: &mut ManagerApp,
    spec: SelectSpec,
    window: &mut Window,
    cx: &mut Context<ManagerApp>,
) -> Stateful<Div> {
    let SelectSpec {
        kind,
        id,
        aria_label,
        current,
        options,
        trigger_width,
        menu_width,
        heading,
    } = spec;
    let enabled = editable(app);
    let is_open = enabled
        && (app.appearance.open_menu == Some(kind)
            || (kind == AppearanceMenu::FontFamily && app.appearance.font_menu_open));
    let selected = options.iter().position(|item| item.value == current);
    let selected_ix = selected.unwrap_or(0);
    let current_value = current.clone();
    let label = if app.appearance.ready {
        selected
            .and_then(|ix| options.get(ix))
            .map(|item| item.label.clone())
            .unwrap_or(current)
    } else {
        "Загрузка…".to_owned()
    };
    let query = if kind == AppearanceMenu::FontFamily && is_open {
        app.input_value("appearance.font.search", cx).to_lowercase()
    } else {
        String::new()
    };
    let mut visible: Vec<OptionItem> = options
        .into_iter()
        .filter(|option| option.label.to_lowercase().contains(&query))
        .collect();
    if kind == AppearanceMenu::FontFamily && !query.is_empty() {
        visible.sort_by_key(|option| !option.label.to_lowercase().starts_with(&query));
    }
    if app.appearance.menu_query != query {
        app.appearance.menu_query = query;
        app.appearance.menu_highlighted = 0;
    }
    app.appearance.menu_highlighted = app
        .appearance
        .menu_highlighted
        .min(visible.len().saturating_sub(1));
    let highlighted = app
        .appearance
        .menu_highlighted
        .min(visible.len().saturating_sub(1));
    let focus = app
        .appearance
        .tile_focus
        .entry(id.to_owned())
        .or_insert_with(|| cx.focus_handle())
        .clone();
    let open_color = fade(FG(), 0.10);
    let base_color = fade(FG(), 0.06);
    let options_for_click = visible.clone();
    let mut trigger = div()
        .id(id)
        .debug_selector(move || id.to_owned())
        .relative()
        .flex_none()
        .max_w_full()
        .w(px(trigger_width))
        .h(px(32.))
        .pl(px(10.))
        .pr(px(8.))
        .rounded(px(8.))
        .border_1()
        .border_color(transparent_black())
        .bg(if is_open { open_color } else { base_color })
        .hover(|style| style.bg(open_color))
        .flex()
        .items_center()
        .gap(px(8.))
        .font_family("Geist")
        .text_size(ui_px(12.5))
        .text_color(c(FG()))
        .role(Role::Button)
        .aria_label(format!("{aria_label}: {label}"))
        .aria_expanded(is_open)
        .track_focus(&focus)
        .tab_index(0)
        .when(enabled, |el| el.cursor_pointer())
        .when(!enabled, |el| el.opacity(0.5))
        .focus_visible(|style| style.border_color(c(ACCENT())))
        .on_click(cx.listener(move |app, event: &ClickEvent, window, cx| {
            if !editable(app) {
                return;
            }
            if event.is_keyboard() && app.appearance.open_menu == Some(kind) {
                let ix = app.appearance.menu_highlighted;
                if let Some(option) = options_for_click.get(ix) {
                    commit(app, kind, option, cx);
                }
                return;
            }
            if app.appearance.open_menu == Some(kind)
                || (kind == AppearanceMenu::FontFamily && app.appearance.font_menu_open)
            {
                close(app, cx);
            } else if !app
                .appearance
                .menu_dismissed_at
                .is_some_and(|(dismissed, at)| {
                    dismissed == kind && at.elapsed() < Duration::from_millis(400)
                })
            {
                open(app, kind, selected_ix, window, cx);
            }
        }))
        .on_key_down({
            let options = visible
                .iter()
                .map(|item| OptionItem {
                    label: item.label.clone(),
                    value: item.value.clone(),
                    palette: item.palette,
                })
                .collect::<Vec<_>>();
            cx.listener(move |app, event: &KeyDownEvent, window, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    return;
                }
                if key_down(
                    app,
                    kind,
                    event.keystroke.key.as_str(),
                    selected_ix,
                    &options,
                    window,
                    cx,
                ) {
                    cx.stop_propagation();
                }
            })
        });
    if let Some(palette) = visible
        .iter()
        .find(|item| item.value == current_value)
        .and_then(|item| item.palette)
    {
        trigger = trigger.child(chip(palette));
    }
    trigger = trigger
        .child(div().flex_1().min_w_0().truncate().child(label))
        .child(
            div()
                .w(px(14.))
                .flex_none()
                .text_size(px(14.))
                .text_color(c(MUTED_FG()))
                .child("⌄"),
        );

    if is_open {
        let mut rows = div()
            .id(format!("{id}-list-{}", app.appearance.menu_generation))
            .debug_selector(move || format!("{id}-list"))
            .max_h(px(if kind == AppearanceMenu::FontFamily {
                240.
            } else {
                300.
            }))
            .overflow_y_scrollbar()
            .flex()
            .flex_col()
            .gap(px(2.));
        if visible.is_empty() {
            rows = rows.child(
                div()
                    .px(px(8.))
                    .py(px(6.))
                    .text_size(ui_px(12.))
                    .text_color(c(MUTED_FG()))
                    .child("Шрифты не найдены"),
            );
        }
        for (ix, item) in visible.iter().enumerate() {
            let value = item.value.clone();
            let label = item.label.clone();
            let palette = item.palette;
            let active = item.value == current_value;
            let row_id = format!("{id}-option-{ix}");
            rows = rows.child(
                div()
                    .id(SharedString::from(row_id.clone()))
                    .debug_selector(move || row_id.clone())
                    .role(Role::MenuItemRadio)
                    .aria_label(label.clone())
                    .aria_toggled(if active {
                        Toggled::True
                    } else {
                        Toggled::False
                    })
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(8.))
                    .py(px(6.))
                    .rounded(px(8.))
                    .text_size(px(13.))
                    .font_family(".SystemUIFont")
                    .text_color(c(FG()))
                    .bg(if active {
                        fade(FG(), 0.10)
                    } else if ix == highlighted {
                        fade(FG(), 0.08)
                    } else {
                        transparent_black()
                    })
                    .hover(|style| style.bg(fade(FG(), 0.08)))
                    .cursor_pointer()
                    .on_click(cx.listener(move |app, _, _, cx| {
                        cx.stop_propagation();
                        commit(
                            app,
                            kind,
                            &OptionItem::new(label.clone(), value.clone()),
                            cx,
                        );
                    }))
                    .children(palette.map(chip))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(item.label.clone()),
                    )
                    .child(
                        div()
                            .w(px(18.))
                            .flex_none()
                            .text_size(px(14.))
                            .text_color(c(ACCENT()))
                            .child(if active { "✓" } else { "" }),
                    ),
            );
        }
        let mut menu = div()
            .w(px(menu_width))
            .max_w_full()
            .max_h(px(((f32::from(window.viewport_size().height) - 32.) / 2.
                - 6.)
                .clamp(1., 320.)))
            .rounded(px(12.))
            .border_1()
            .border_color(c(BORDER()))
            .shadow_lg()
            .bg(c(CARD()))
            .p(px(4.))
            .font_family(".SystemUIFont")
            .flex()
            .flex_col()
            .gap(px(4.))
            .on_mouse_down_out(cx.listener(move |app, _, _, cx| close(app, cx)))
            .on_key_down({
                let options = visible
                    .iter()
                    .map(|item| OptionItem {
                        label: item.label.clone(),
                        value: item.value.clone(),
                        palette: item.palette,
                    })
                    .collect::<Vec<_>>();
                cx.listener(move |app, event: &KeyDownEvent, window, cx| {
                    if key_down(
                        app,
                        kind,
                        event.keystroke.key.as_str(),
                        selected_ix,
                        &options,
                        window,
                        cx,
                    ) {
                        cx.stop_propagation();
                    }
                })
            });
        if let Some(heading) = heading {
            menu = menu.child(
                div()
                    .px(px(8.))
                    .pt(px(6.))
                    .pb(px(4.))
                    .text_size(ui_px(10.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(c(MUTED_FG()))
                    .child(heading.to_uppercase()),
            );
        }
        if kind == AppearanceMenu::FontFamily {
            let input = watched_input(app, "appearance.font.search", "Поиск шрифта", window, cx);
            menu = menu.child(
                div().px(px(10.)).py(px(6.)).mb(px(4.)).child(
                    gpui_component::input::Input::new(&input)
                        .h(px(32.))
                        .text_size(px(12.5))
                        .font_family(".SystemUIFont"),
                ),
            );
        }
        menu = menu.child(rows);
        trigger = trigger.child(
            div().absolute().bottom_0().right_0().size_0().child(
                deferred(
                    anchored()
                        .anchor(Anchor::TopRight)
                        .snap_to_window_with_margin(px(8.))
                        .child(div().occlude().pt(px(6.)).child(menu)),
                )
                .priority(1),
            ),
        );
    }
    trigger
}
