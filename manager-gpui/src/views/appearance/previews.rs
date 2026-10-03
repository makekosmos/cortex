//! Visuals adapted from Zeron's MIT-licensed appearance settings (2026 Wing).
use super::*;
use crate::appearance_state::AppearanceMenu;
use imago_gpui::theme::Palette;

fn bar(fraction: f32, color: Hsla) -> Div {
    div()
        .h(px(5.))
        .w(relative(fraction))
        .rounded(px(3.))
        .bg(color)
}

#[derive(Clone, Copy)]
enum Corners {
    All,
    Left,
    Right,
}

fn miniature(palette: &Palette, corners: Corners) -> Div {
    let line = fade(palette.fg, 0.22);
    let strong = fade(palette.fg, 0.34);
    let root = div().size_full().flex().bg(c(palette.card));
    let root = match corners {
        Corners::All => root.rounded(px(6.)),
        Corners::Left => root.rounded_tl(px(6.)).rounded_bl(px(6.)),
        Corners::Right => root.rounded_tr(px(6.)).rounded_br(px(6.)),
    };
    root.child(
        div()
            .w(px(44.))
            .h_full()
            .flex_none()
            .overflow_hidden()
            .flex()
            .flex_col()
            .gap(px(7.))
            .px(px(8.))
            .pt(px(14.))
            .child(bar(0.70, strong))
            .child(bar(1., line))
            .child(bar(0.85, line))
            .child(bar(1., line)),
    )
    .child(
        div()
            .flex_1()
            .min_w_0()
            .my(px(8.))
            .mr(px(8.))
            .rounded(px(6.))
            .border_1()
            .border_color(c(palette.border))
            .bg(c(palette.bg))
            .overflow_hidden()
            .flex()
            .flex_col()
            .gap(px(7.))
            .p(px(10.))
            .child(bar(0.62, strong))
            .child(bar(0.88, line))
            .child(bar(0.76, line))
            .child(bar(0.52, line)),
    )
}

fn mode_card(
    app: &mut ManagerApp,
    mode: &'static str,
    label: &'static str,
    icon: &'static str,
    preview: impl IntoElement,
    cx: &mut Context<ManagerApp>,
) -> Stateful<Div> {
    let id = format!("appearance-mode-{mode}");
    let focus = app
        .appearance
        .tile_focus
        .entry(id.clone())
        .or_insert_with(|| cx.focus_handle())
        .clone();
    let selected = app.appearance.ready && app.appearance.settings.mode == mode;
    let enabled = editable(app);
    let edge = if selected { ACCENT() } else { BORDER() };
    let caption = if selected { ACCENT() } else { MUTED_FG() };
    div()
        .id(SharedString::from(id.clone()))
        .debug_selector(move || id.clone())
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(8.))
        .role(Role::Button)
        .aria_label(format!("Режим темы: {label}"))
        .aria_selected(selected)
        .track_focus(&focus)
        .tab_index(0)
        .when(enabled, |el| el.cursor_pointer())
        .when(!enabled, |el| el.opacity(0.5))
        .focus_visible(|style| style.border_color(c(ACCENT())))
        .on_click(cx.listener(move |app, _, window, cx| {
            if editable(app) {
                window.focus(&focus, cx);
                patch(app, json!({"mode":mode}), cx);
            }
        }))
        .on_key_down(cx.listener(move |app, event: &KeyDownEvent, _, cx| {
            if matches!(event.keystroke.key.as_str(), "enter" | "space") && editable(app) {
                patch(app, json!({"mode":mode}), cx);
                cx.stop_propagation();
            }
        }))
        .child(
            div()
                .h(px(148.))
                .flex_none()
                .w_full()
                .rounded(px(6.))
                .overflow_hidden()
                .border_1()
                .border_color(c(edge))
                .child(preview),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .font_family(".SystemUIFont")
                .text_size(px(13.))
                .line_height(px(16.))
                .font_weight(if selected {
                    FontWeight::MEDIUM
                } else {
                    FontWeight::NORMAL
                })
                .text_color(c(caption))
                .child(svg().path(icon).size(px(16.)).text_color(c(caption)))
                .child(label),
        )
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
    let split = div()
        .size_full()
        .flex()
        .child(
            div()
                .w_1_2()
                .h_full()
                .overflow_hidden()
                .child(miniature(&light.light, Corners::Left)),
        )
        .child(
            div()
                .w_1_2()
                .h_full()
                .overflow_hidden()
                .child(miniature(&dark.dark, Corners::Right)),
        );
    section_group()
        .child(section(
            "Режим темы",
            "Системная тема следует светлому или тёмному режиму ОС",
        ))
        .child(
            div()
                .w_full()
                .flex()
                .items_start()
                .gap(px(16.))
                .child(mode_card(
                    app,
                    "system",
                    "Системная",
                    "icons/monitor.svg",
                    split,
                    cx,
                ))
                .child(mode_card(
                    app,
                    "light",
                    "Светлая",
                    "icons/sun.svg",
                    miniature(&light.light, Corners::All),
                    cx,
                ))
                .child(mode_card(
                    app,
                    "dark",
                    "Тёмная",
                    "icons/moon.svg",
                    miniature(&dark.dark, Corners::All),
                    cx,
                )),
        )
}

pub(super) fn palettes(
    app: &mut ManagerApp,
    window: &mut Window,
    cx: &mut Context<ManagerApp>,
) -> Div {
    let mut rows = card()
        .id("appearance-theme-card")
        .debug_selector(|| "appearance-theme-card".into())
        .gap(px(0.));
    for (ix, (kind, title, field, dark)) in [
        (
            AppearanceMenu::LightTheme,
            "Светлая тема",
            "light_theme",
            false,
        ),
        (AppearanceMenu::DarkTheme, "Тёмная тема", "dark_theme", true),
    ]
    .into_iter()
    .enumerate()
    {
        let selected = if dark {
            app.appearance.settings.dark_theme.clone()
        } else {
            app.appearance.settings.light_theme.clone()
        };
        let options = imago_gpui::THEMES
            .iter()
            .map(|theme| {
                let palette = if dark { &theme.dark } else { &theme.light };
                selector::OptionItem::new(theme.name, theme.key).palette(
                    palette.card,
                    palette.bg,
                    palette.accent,
                    palette.border,
                )
            })
            .collect();
        let selector = selector::select(
            app,
            selector::SelectSpec {
                kind,
                id: if dark {
                    "dark-theme-selector"
                } else {
                    "light-theme-selector"
                },
                aria_label: title,
                current: selected,
                options,
                trigger_width: 218.,
                menu_width: 260.,
                heading: Some(if dark {
                    "Тёмные темы"
                } else {
                    "Светлые темы"
                }),
            },
            window,
            cx,
        );
        rows = rows.child(
            div()
                .min_h(px(60.))
                .py(px(12.))
                .when(ix != 0, |row| row.border_t_1().border_color(c(BORDER())))
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(16.))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(160.))
                        .text_size(ui_px(13.))
                        .font_weight(FontWeight::MEDIUM)
                        .child(title),
                )
                .child(selector)
                .debug_selector(move || format!("appearance-{field}-row")),
        );
    }
    section_group()
        .child(section(
            "Цветовые темы",
            "Отдельная палитра для каждого режима",
        ))
        .child(rows)
}
