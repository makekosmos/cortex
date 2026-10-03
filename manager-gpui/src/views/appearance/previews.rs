//! Rectangular mode previews and independently selected light/dark palettes.
use super::*;
use imago_gpui::theme::Palette;

fn miniature(palette: &Palette, height: f32) -> Div {
    div()
        .w_full()
        .min_w_0()
        .h(px(height))
        .flex()
        .rounded(px(6.))
        .overflow_hidden()
        .border_1()
        .border_color(c(palette.border))
        .bg(c(palette.bg))
        .child(
            div()
                .w(px(24.))
                .h_full()
                .bg(c(palette.sidebar_bg))
                .p(px(6.))
                .flex()
                .flex_col()
                .gap(px(5.))
                .child(div().h(px(4.)).w_full().rounded_sm().bg(c(palette.accent)))
                .child(
                    div()
                        .h(px(4.))
                        .w_full()
                        .rounded_sm()
                        .bg(fade(palette.fg, 0.2)),
                ),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .p(px(8.))
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(div().h(px(5.)).w(px(24.)).rounded_sm().bg(c(palette.fg)))
                .child(
                    div()
                        .flex_1()
                        .w_full()
                        .rounded_sm()
                        .bg(c(palette.card))
                        .border_1()
                        .border_color(c(palette.border)),
                )
                .child(
                    div()
                        .h(px(6.))
                        .w(px(32.))
                        .rounded_sm()
                        .bg(c(palette.accent)),
                ),
        )
}

fn tile(
    app: &mut ManagerApp,
    id: String,
    label: String,
    selected: bool,
    params: Value,
    cx: &mut Context<ManagerApp>,
) -> Stateful<Div> {
    let enabled = editable(app);
    let focus = app
        .appearance
        .tile_focus
        .entry(id.clone())
        .or_insert_with(|| cx.focus_handle())
        .clone();
    let key_params = params.clone();
    div()
        .id(SharedString::from(id))
        .role(Role::Button)
        .aria_label(label)
        .aria_selected(selected)
        .aria_description(if enabled {
            ""
        } else {
            "Настройка пока недоступна"
        })
        .track_focus(&focus)
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(8.))
        .p(px(10.))
        .rounded(px(9.))
        .border_1()
        .border_color(c(if selected { ACCENT() } else { BORDER() }))
        .bg(c(CARD()))
        .when(enabled, |d| d.cursor_pointer())
        .when(!enabled, |d| d.opacity(0.6))
        .focus(|style| style.border_color(c(ACCENT())))
        .on_click(cx.listener(move |app, _, window, cx| {
            if editable(app) {
                window.focus(&focus, cx);
                patch(app, params.clone(), cx);
            }
        }))
        .on_key_down(cx.listener(move |app, event: &KeyDownEvent, _, cx| {
            if matches!(event.keystroke.key.as_str(), "enter" | "space") && editable(app) {
                patch(app, key_params.clone(), cx);
                cx.stop_propagation();
            }
        }))
}

fn caption(label: &str) -> Div {
    div()
        .min_w_0()
        .text_size(ui_px(13.))
        .line_height(ui_px(18.))
        .text_ellipsis()
        .whitespace_nowrap()
        .overflow_hidden()
        .child(label.to_owned())
}

pub(super) fn modes(app: &mut ManagerApp, cx: &mut Context<ManagerApp>) -> Div {
    let themes = imago_gpui::THEMES;
    let light = themes
        .iter()
        .find(|t| t.key == app.appearance.settings.light_theme)
        .unwrap_or(&themes[0]);
    let dark = themes
        .iter()
        .find(|t| t.key == app.appearance.settings.dark_theme)
        .unwrap_or(&themes[0]);
    let mut tiles = div().w_full().min_w_0().grid().grid_cols(3).gap(px(12.));
    for (mode, label) in [
        ("system", "Системная"),
        ("light", "Светлая"),
        ("dark", "Тёмная"),
    ] {
        let preview = match mode {
            "light" => miniature(&light.light, 100.),
            "dark" => miniature(&dark.dark, 100.),
            _ => div()
                .w_full()
                .min_w_0()
                .flex()
                .gap(px(2.))
                .child(miniature(&light.light, 100.))
                .child(miniature(&dark.dark, 100.)),
        };
        let selected = app.appearance.ready && app.appearance.settings.mode == mode;
        tiles = tiles.child(
            tile(
                app,
                format!("appearance-mode-{mode}"),
                format!("Режим темы: {label}"),
                selected,
                json!({"mode":mode}),
                cx,
            )
            .debug_selector(move || format!("appearance-mode-{mode}"))
            .child(preview)
            .child(caption(label)),
        );
    }
    section_group()
        .child(section(
            "Режим темы",
            "Системная тема следует светлому или тёмному режиму ОС",
        ))
        .child(tiles)
}

pub(super) fn palettes(app: &mut ManagerApp, cx: &mut Context<ManagerApp>) -> Div {
    let mut columns = div().w_full().min_w_0().grid().grid_cols(2).gap(px(12.));
    for (field, label, dark) in [
        ("light_theme", "Светлая тема", false),
        ("dark_theme", "Тёмная тема", true),
    ] {
        let selected_key = if dark {
            app.appearance.settings.dark_theme.clone()
        } else {
            app.appearance.settings.light_theme.clone()
        };
        let mut choices = div().w_full().min_w_0().grid().grid_cols(2).gap(px(8.));
        for theme in imago_gpui::THEMES {
            let palette = if dark { &theme.dark } else { &theme.light };
            let params = json!({(field):theme.key});
            choices = choices.child(
                tile(
                    app,
                    format!("{field}-{}", theme.key),
                    format!("{label}: {}", theme.name),
                    app.appearance.ready && selected_key == theme.key,
                    params,
                    cx,
                )
                .child(miniature(palette, 76.))
                .child(caption(theme.name)),
            );
        }
        columns = columns.child(section_group().child(section(label, "")).child(choices));
    }
    section_group()
        .child(section(
            "Цветовые темы",
            "Отдельная палитра для каждого режима",
        ))
        .child(columns)
}
