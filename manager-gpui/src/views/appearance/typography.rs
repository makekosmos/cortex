//! One Engine-backed interface font row, using Zeron's selector geometry.
use super::*;
use crate::appearance_state::AppearanceMenu;

pub(super) fn render(
    app: &mut ManagerApp,
    window: &mut Window,
    cx: &mut Context<ManagerApp>,
) -> Div {
    let family = app.appearance.settings.font_family.clone();
    let mut options: Vec<_> = app
        .appearance
        .fonts
        .iter()
        .map(|font| {
            selector::OptionItem::new(
                if font == ".SystemUIFont" {
                    "Системный"
                } else {
                    font
                },
                font,
            )
        })
        .collect();
    options.sort_by_key(|item| item.label.to_lowercase());
    let font = selector::select(
        app,
        selector::SelectSpec {
            kind: AppearanceMenu::FontFamily,
            id: "appearance-font-picker",
            aria_label: "Шрифт интерфейса",
            current: family.clone(),
            options,
            trigger_width: 220.,
            menu_width: 220.,
            heading: None,
        },
        window,
        cx,
    );
    // The Engine accepts 11..=18 and defaults to 13. Zeron's 20px rung is
    // unavailable in this protocol; 12.5 remains selectable for saved values.
    let sizes = [11., 12., 12.5, 13., 14., 15., 16., 18.]
        .into_iter()
        .map(|size| selector::OptionItem::new(format!("{size} px"), format!("{size}")))
        .collect();
    let size = selector::select(
        app,
        selector::SelectSpec {
            kind: AppearanceMenu::FontSize,
            id: "appearance-font-size-dropdown",
            aria_label: "Размер шрифта",
            current: format!("{}", app.appearance.settings.font_size),
            options: sizes,
            trigger_width: 128.,
            menu_width: 128.,
            heading: None,
        },
        window,
        cx,
    );
    let mut card = card()
        .id("appearance-font-card")
        .debug_selector(|| "appearance-font-card".into())
        .child(
            div()
                .min_h(px(60.))
                .py(px(12.))
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(16.))
                .child(
                    div()
                        .min_w(px(160.))
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(
                            div()
                                .text_size(ui_px(13.))
                                .line_height(ui_px(17.))
                                .font_weight(FontWeight::MEDIUM)
                                .child("Шрифт интерфейса"),
                        )
                        .child(
                            div()
                                .text_size(ui_px(12.))
                                .line_height(ui_px(16.))
                                .text_color(c(MUTED_FG()))
                                .child("Список шрифтов, доступных на этом устройстве."),
                        ),
                )
                .child(
                    div()
                        .flex_none()
                        .max_w_full()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap(px(8.))
                        .child(font)
                        .child(size),
                ),
        );
    if app.appearance.ready && !app.appearance.fonts.contains(&family) {
        card = card.child(empty("Выбранный шрифт недоступен; используется системный."));
    }
    section_group()
        .child(section(
            "Типографика",
            "Шрифт и размер текста, без изменения сетки отступов",
        ))
        .child(card)
}
