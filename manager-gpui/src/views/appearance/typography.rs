use super::*;
use gpui_component::scroll::ScrollableElement;

pub(super) fn render(
    app: &mut ManagerApp,
    window: &mut Window,
    cx: &mut Context<ManagerApp>,
) -> Div {
    let enabled = editable(app);
    let family = app.appearance.settings.font_family.clone();
    let label = if !app.appearance.ready {
        "Загрузка…"
    } else if family == ".SystemUIFont" {
        "Системный"
    } else {
        &family
    };
    let mut content = card()
        .id("appearance-font-card")
        .debug_selector(|| "appearance-font-card".into())
        .child(
            row(
                "Шрифт интерфейса",
                "Список шрифтов, доступных на этом устройстве.",
            )
            .child(
                crate::button::button("appearance-font-picker", crate::button::ButtonKind::Ghost)
                    .label(label.to_owned())
                    .disabled(!enabled)
                    .on_click(cx.listener(|app, _, _, cx| {
                        app.appearance.font_menu_open = !app.appearance.font_menu_open;
                        cx.notify();
                    })),
            ),
        );
    if app.appearance.font_menu_open {
        let search = watched_input(app, "appearance.font.search", "Поиск шрифта", window, cx);
        let query = app.input_value("appearance.font.search", cx).to_lowercase();
        let matches: Vec<_> = app
            .appearance
            .fonts
            .iter()
            .filter(|name| name.to_lowercase().contains(&query))
            .cloned()
            .collect();
        let mut list = div()
            .id("appearance-font-list")
            .w_full()
            .min_w_0()
            .max_h(px(240.))
            .overflow_y_scrollbar()
            .flex()
            .flex_col()
            .gap(px(4.));
        for name in matches.iter().take(60) {
            let selected = family == *name;
            let display = if name == ".SystemUIFont" {
                "Системный"
            } else {
                name.as_str()
            };
            let chosen = name.clone();
            list = list.child(
                choice(
                    app,
                    format!("font-{name}"),
                    display,
                    selected,
                    json!({"font_family":name}),
                    cx,
                )
                .w_full()
                .font_family(name.clone())
                .on_click(cx.listener(move |app, _, _, cx| {
                    patch(app, json!({"font_family":chosen}), cx);
                    app.appearance.font_menu_open = false;
                })),
            );
        }
        content = content
            .child(input_field(&search).disabled(!enabled))
            .child(list);
        if matches.len() > 60 {
            content = content.child(empty("Показаны первые 60 шрифтов. Уточните поиск."));
        } else if matches.is_empty() {
            content = content.child(empty("Шрифты не найдены."));
        }
    }
    if app.appearance.ready && !app.appearance.fonts.contains(&family) {
        content = content.child(empty("Выбранный шрифт недоступен; используется системный."));
    }
    let mut sizes = div().flex().flex_wrap().gap(px(4.));
    for size in [11., 12., 12.5, 13., 14., 15., 16., 18.] {
        sizes = sizes.child(choice(
            app,
            format!("font-size-{size}"),
            &format!("{size}"),
            app.appearance.ready && app.appearance.settings.font_size == size,
            json!({"font_size":size}),
            cx,
        ));
    }
    content =
        content.child(row("Размер шрифта", "Размер основного текста в пикселях.").child(sizes));
    content = content.child(
        div()
            .w_full()
            .min_w_0()
            .py(px(8.))
            .text_size(ui_px(13.))
            .line_height(ui_px(18.))
            .child("Пример текста: Съешь ещё этих мягких французских булок. 0123456789"),
    );
    section_group()
        .child(section(
            "Типографика",
            "Шрифт и размер текста, без изменения сетки отступов",
        ))
        .child(content)
}
