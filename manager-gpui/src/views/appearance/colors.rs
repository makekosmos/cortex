use super::*;
use crate::appearance_state::parse_color;

pub(super) fn render(
    app: &mut ManagerApp,
    window: &mut Window,
    cx: &mut Context<ManagerApp>,
) -> Div {
    let source = app.appearance.settings.accent_source.clone();
    let mut sources = div().flex().flex_wrap().gap(px(8.));
    for (key, label) in [
        ("theme", "Цвет темы"),
        ("custom", "Свой цвет"),
        ("wallpaper", "Из обоев рабочего стола"),
    ] {
        let disabled =
            !editable(app) || (key == "wallpaper" && !app.appearance.wallpaper_supported);
        let params = if key == "custom" {
            json!({"accent_source":key, "accent_color":app.appearance.settings.accent_color
                .clone().unwrap_or_else(|| format!("#{:06X}", ACCENT()))})
        } else {
            json!({"accent_source":key})
        };
        sources = sources.child(
            choice(
                app,
                format!("accent-source-{key}"),
                label,
                app.appearance.ready && source == key,
                params,
                cx,
            )
            .disabled(disabled),
        );
    }
    let mut content = card()
        .id("appearance-accent-card")
        .debug_selector(|| "appearance-accent-card".into())
        .child(sources);
    if source == "custom" {
        let current = app.appearance.settings.accent_color.clone();
        let mut swatches = div().flex().flex_wrap().gap(px(8.));
        for color in [
            "#7C5CFC", "#3B82F6", "#14B8A6", "#22C55E", "#EAB308", "#F97316", "#EF4444", "#EC4899",
        ] {
            swatches = swatches.child(
                choice(
                    app,
                    format!("accent-{color}"),
                    color,
                    current.as_deref() == Some(color),
                    json!({"accent_source":"custom","accent_color":color}),
                    cx,
                )
                .bg(c(parse_color(color).expect("literal color")))
                .text_color(c(crate::theme::accent_foreground(
                    parse_color(color).unwrap(),
                ))),
            );
        }
        let input = watched_input(
            app,
            "appearance.accent.hex",
            "Цвет HEX, например #7C5CFC",
            window,
            cx,
        );
        let value = app.input_value("appearance.accent.hex", cx);
        let valid = parse_color(&value).is_some();
        let row = div()
            .w_full()
            .min_w_0()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(
                input_field(&input)
                    .flex_1()
                    .min_w_0()
                    .disabled(!editable(app)),
            )
            .child(
                crate::button::button("accent-apply", crate::button::ButtonKind::Ghost)
                    .label("Применить HEX")
                    .disabled(!editable(app) || !valid)
                    .on_click(cx.listener(|app, _, _, cx| {
                        let value = app.input_value("appearance.accent.hex", cx);
                        if parse_color(&value).is_some() {
                            patch(
                                app,
                                json!({"accent_source":"custom","accent_color":value}),
                                cx,
                            );
                        }
                    })),
            );
        content = content.child(swatches).child(row);
        if !value.is_empty() && !valid {
            content = content.child(empty("Введите # и шесть шестнадцатеричных цифр."));
        }
    }
    if !app.appearance.wallpaper_supported {
        content = content.child(empty(
            "Извлечение акцента из обоев недоступно на этом устройстве.",
        ));
    } else if source == "wallpaper" {
        let message = match app.appearance.wallpaper_error.as_deref() {
            Some("desktop wallpaper accent is pending") => "Определяем цвет обоев…",
            Some(_) => {
                "Обои недоступны. Проверьте разрешения Engine. Пока используется акцент темы."
            }
            None if app.appearance.wallpaper_accent.is_some() => {
                "Акцент взят из текущей картинки рабочего стола."
            }
            None => "Цвет обоев пока недоступен; используется акцент темы.",
        };
        content = content.child(empty(message));
    }
    if app.appearance.ready {
        content = content.child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(
                    div()
                        .size(px(24.))
                        .rounded_full()
                        .bg(c(ACCENT()))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(ui_px(11.))
                        .text_color(c(ACCENT_FG()))
                        .child("Aa"),
                )
                .child(
                    div()
                        .text_size(ui_px(12.))
                        .line_height(ui_px(16.))
                        .text_color(c(MUTED_FG()))
                        .child(format!("Текущий акцент: #{:06X}", ACCENT())),
                ),
        );
    }
    section_group()
        .child(section(
            "Акцентный цвет",
            "По умолчанию используется акцент выбранной темы",
        ))
        .child(content)
}
