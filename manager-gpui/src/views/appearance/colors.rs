//! Accent row copied from Zeron's swatch control. Preset values are the same
//! published colors; selecting one stores Engine `accent_source=custom`.
use super::*;
use crate::appearance_state::parse_color;

const PRESETS: &[(&str, u32, u32)] = &[
    ("Zeron", 0x8b7cf6, 0x5b43e8),
    ("Orange", 0xfb923c, 0xc2410c),
    ("Amber", 0xfbbf24, 0xa16207),
    ("Green", 0x4ade80, 0x15803d),
    ("Cyan", 0x22d3ee, 0x0e7490),
    ("Blue", 0x60a5fa, 0x2563eb),
    ("Pink", 0xf472b6, 0xbe185d),
];

fn swatch(
    app: &mut ManagerApp,
    id: String,
    label: &str,
    selected: bool,
    sample: Div,
    params: Value,
    cx: &mut Context<ManagerApp>,
) -> Stateful<Div> {
    let enabled = editable(app);
    div()
        .id(SharedString::from(id.clone()))
        .debug_selector(move || id)
        .aria_label(label.to_owned())
        .aria_selected(selected)
        .flex_none()
        .w(px(30.))
        .h(px(34.))
        .pb(px(4.))
        .border_b_2()
        .border_color(if selected {
            c(ACCENT())
        } else {
            transparent_black()
        })
        .when(enabled, |el| el.cursor_pointer())
        .when(!enabled, |el| el.opacity(0.5))
        .child(
            div()
                .size(px(30.))
                .p(px(2.))
                .rounded(px(8.))
                .border_1()
                .border_color(c(if selected { FG() } else { BORDER() }))
                .bg(fade(CARD(), 0.42))
                .child(sample),
        )
        .on_click(cx.listener(move |app, _, _, cx| patch(app, params.clone(), cx)))
}

pub(super) fn accent_controls(app: &mut ManagerApp, cx: &mut Context<ManagerApp>) -> Div {
    let dark = app.appearance.settings.mode == "dark";
    let theme_selected = app.appearance.ready && app.appearance.settings.accent_source == "theme";
    let custom = app
        .appearance
        .settings
        .accent_color
        .as_deref()
        .and_then(parse_color);
    let mut row = div()
        .max_w_full()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(8.))
        .child(swatch(
            app,
            "accent-theme".into(),
            "Цвет темы",
            theme_selected,
            div()
                .size_full()
                .rounded(px(6.))
                .bg(fade(ACCENT(), 0.22))
                .flex()
                .items_center()
                .justify_center()
                .gap(px(2.))
                .child(
                    div()
                        .w(px(4.))
                        .h(px(13.))
                        .rounded(px(2.))
                        .bg(fade(ACCENT(), 0.45)),
                )
                .child(div().w(px(4.)).h(px(16.)).rounded(px(2.)).bg(c(ACCENT())))
                .child(
                    div()
                        .w(px(4.))
                        .h(px(11.))
                        .rounded(px(2.))
                        .bg(fade(ACCENT(), 0.72)),
                ),
            json!({"accent_source":"theme","accent_color":Value::Null}),
            cx,
        ));
    for (name, dark_color, light_color) in PRESETS {
        let color = if dark { *dark_color } else { *light_color };
        let selected = app.appearance.ready
            && app.appearance.settings.accent_source == "custom"
            && custom == Some(color);
        row = row.child(swatch(
            app,
            format!("accent-{name}"),
            name,
            selected,
            div().size_full().rounded(px(6.)).bg(c(color)),
            json!({"accent_source":"custom","accent_color":format!("#{color:06X}")}),
            cx,
        ));
    }
    row
}

pub(super) fn wallpaper_meta(app: &ManagerApp) -> &'static str {
    if !app.appearance.wallpaper_supported {
        return "Извлечение акцента из обоев недоступно на этом устройстве.";
    }
    match app.appearance.wallpaper_error.as_deref() {
        Some("desktop wallpaper accent is pending") => "Определяем цвет обоев…",
        Some(_) => "Обои недоступны. Пока используется акцент темы.",
        None if app.appearance.settings.accent_source == "wallpaper"
            && app.appearance.wallpaper_accent.is_some() =>
        {
            "Акцент взят из текущей картинки рабочего стола."
        }
        None => "Использовать цвета обоев для акцента и подсветок.",
    }
}
