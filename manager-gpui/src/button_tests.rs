use super::{button, caption, ButtonKind, LABEL_LINE_HEIGHT, LABEL_SIZE};
use gpui::{div, prelude::*, px, Context, Render, Styled, TestAppContext, Window};

#[test]
fn caption_uses_explicit_typography_not_parent_or_medium_button_defaults() {
    let mut text = caption("Отключить".into());
    assert_eq!(text.style().text.font_size, Some(px(LABEL_SIZE).into()));
    assert_eq!(
        text.style().text.line_height,
        Some(px(LABEL_LINE_HEIGHT).into())
    );
}

struct Harness {
    kind: ButtonKind,
    disabled: bool,
    clicks: usize,
}

impl Render for Harness {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .text_size(px(24.))
            .flex()
            .items_start()
            .p_4()
            .child(
                div()
                    .id("button-holder")
                    .debug_selector(|| "button-holder".into())
                    .child(
                        button("action", self.kind)
                            .label("Отключить")
                            .disabled(self.disabled)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.clicks += 1;
                                cx.notify();
                            })),
                    ),
            )
    }
}

#[gpui::test]
fn smaller_caption_keeps_control_height_and_click_disabled_behavior(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    cx.update(imago_gpui::theme::apply);
    for kind in [
        ButtonKind::Primary,
        ButtonKind::Secondary,
        ButtonKind::Ghost,
        ButtonKind::Danger,
        ButtonKind::Success,
    ] {
        let (harness, cx) = cx.add_window_view(|_, _| Harness {
            kind,
            disabled: false,
            clicks: 0,
        });
        let control = cx.debug_bounds("button-holder").unwrap();
        let text = cx.debug_bounds("manager-button-caption").unwrap();
        assert_eq!(
            control.size.height,
            px(32.),
            "Medium hit target must not shrink"
        );
        assert_eq!(text.size.height, px(LABEL_LINE_HEIGHT));
        cx.update(|window, cx| {
            window.focus_next(cx);
            assert!(window.focused(cx).is_some(), "button must remain focusable");
        });
        cx.simulate_click(control.center(), Default::default());
        assert_eq!(harness.read_with(cx, |h, _| h.clicks), 1);
        harness.update(cx, |h, cx| {
            h.disabled = true;
            cx.notify();
        });
        cx.update(|_, cx| cx.refresh_windows());
        cx.simulate_click(control.center(), Default::default());
        assert_eq!(
            harness.read_with(cx, |h, _| h.clicks),
            1,
            "disabled action must not fire"
        );
    }
}

#[test]
fn manager_views_do_not_bypass_shared_button_component() {
    fn check(directory: &std::path::Path) {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                check(&path);
                continue;
            }
            if path.extension().and_then(|v| v.to_str()) != Some("rs") {
                continue;
            }
            if matches!(
                path.file_name().and_then(|v| v.to_str()),
                Some("button.rs" | "button_tests.rs")
            ) {
                continue;
            }
            let source = std::fs::read_to_string(&path).unwrap();
            for bypass in ["imago_gpui::button", "gpui_component::button::Button"] {
                assert!(
                    !source.contains(bypass),
                    "{} bypasses common button typography",
                    path.display()
                );
            }
        }
    }
    check(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"));
}
