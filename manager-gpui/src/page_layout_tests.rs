use super::{card, input_field, key_column, page_stack, row, INSET, LABEL_WIDTH, PAGE_WIDTH};
use ::gpui::{div, prelude::*, px, Context, Entity, Render, Styled, TestAppContext, Window};
use gpui_component::input::InputState;

struct Harness {
    input: Entity<InputState>,
}
impl Render for Harness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        page_stack()
            .w(px(PAGE_WIDTH))
            .text_size(px(24.))
            .child(
                card()
                    .id("ruler-card-a")
                    .debug_selector(|| "ruler-card-a".into())
                    .child(
                        row("Короткое имя", "Описание")
                            .id("ruler-row-a")
                            .debug_selector(|| "ruler-row-a".into())
                            .child(
                                div()
                                    .id("ruler-control-a")
                                    .debug_selector(|| "ruler-control-a".into())
                                    .flex_none()
                                    .child(crate::button::secondary("save-a").label("Сохранить")),
                            ),
                    )
                    .child(
                        row("Очень длинное имя параметра ".repeat(8), "")
                            .id("ruler-row-b")
                            .debug_selector(|| "ruler-row-b".into())
                            .child(
                                div()
                                    .id("ruler-control-b")
                                    .debug_selector(|| "ruler-control-b".into())
                                    .w(px(120.))
                                    .flex_none()
                                    .child(input_field(&self.input)),
                            ),
                    ),
            )
            .child(
                card()
                    .id("ruler-card-b")
                    .debug_selector(|| "ruler-card-b".into())
                    .child(
                        row("Другой параметр", "Другая подпись")
                            .child(crate::button::secondary("save-b").label("Сохранить")),
                    ),
            )
    }
}

#[gpui::test]
fn cards_rows_inputs_and_actions_share_exact_grid(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    cx.update(imago_gpui::theme::apply);
    let (_, cx) = cx.add_window_view(|window, cx| Harness {
        input: cx.new(|cx| InputState::new(window, cx)),
    });
    let a = cx.debug_bounds("ruler-card-a").unwrap();
    let b = cx.debug_bounds("ruler-card-b").unwrap();
    assert_eq!(a.origin.x, b.origin.x);
    assert_eq!(a.size.width, b.size.width);
    assert_eq!(b.origin.y - a.bottom(), px(24.));
    for (row, control) in [
        ("ruler-row-a", "ruler-control-a"),
        ("ruler-row-b", "ruler-control-b"),
    ] {
        let row = cx.debug_bounds(row).unwrap();
        let control = cx.debug_bounds(control).unwrap();
        assert_eq!(row.origin.x - a.origin.x, px(INSET + 1.));
        assert_eq!(a.right() - control.right(), px(INSET + 1.));
        assert_eq!(row.center().y, control.center().y);
        assert_eq!(control.size.height, px(32.));
        assert_eq!(
            row.size.height,
            px(36.),
            "long names must not push controls off grid"
        );
    }
}

#[test]
fn field_labels_keep_one_width_independent_of_caption_length() {
    for name in ["ОС", "Очень длинная подпись поля"] {
        let mut column = key_column(name);
        assert_eq!(column.style().size.width, Some(px(LABEL_WIDTH).into()));
        assert_eq!(column.style().text.line_height, Some(px(18.).into()));
    }
}
