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
    let mut block = section_block(
        "Шрифты",
        settings_card()
            .id("appearance-font-card")
            .debug_selector(|| "appearance-font-card".into())
            .child(
                card_row(true)
                    .child(
                        div()
                            .min_w(px(160.))
                            .flex_1()
                            .child(row_title("Шрифт интерфейса"))
                            .child(row_meta("Меню и текст интерфейса.")),
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
            ),
    );
    if app.appearance.ready && !app.appearance.fonts.contains(&family) {
        block = block.child(empty("Выбранный шрифт недоступен; используется системный."));
    }
    block
}
