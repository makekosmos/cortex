use super::{parse_color, Appearance, Settings};
use crate::app::Slot;
use crate::views::View;
use gpui::{px, size, TestAppContext};
use serde_json::json;

#[gpui::test]
fn semantic_sections_have_more_space_than_their_heading_to_card(cx: &mut TestAppContext) {
    let (manager, cx) = crate::a11y_tests::launch(cx);
    cx.simulate_resize(size(px(1440.), px(2400.)));
    manager.update(cx, |app, cx| {
        app.view = View::Settings;
        cx.notify();
    });
    cx.update(|_, cx| cx.refresh_windows());
    for (previous, next) in [
        ("settings-general-group", "settings-system-group"),
        ("settings-system-group", "settings-privacy-group"),
        ("settings-privacy-group", "settings-developer-group"),
    ] {
        let previous = cx.debug_bounds(previous).unwrap();
        let next = cx.debug_bounds(next).unwrap();
        assert_eq!(next.origin.y - previous.bottom(), px(32.));
    }
    let heading = cx.debug_bounds("settings-general-heading").unwrap();
    let card = cx.debug_bounds("settings-startup-card").unwrap();
    assert_eq!(card.origin.y - heading.bottom(), px(12.));
}

#[test]
fn colors_require_exact_safe_hex_and_choose_readable_foregrounds() {
    assert_eq!(parse_color("#123aBC"), Some(0x123abc));
    for invalid in ["", "123abc", "#abc", "#12345678", "#gg0011", "#😀123"] {
        assert_eq!(parse_color(invalid), None);
    }
    assert_eq!(crate::theme::accent_foreground(0xffffff), 0x000000);
    assert_eq!(crate::theme::accent_foreground(0x000000), 0xffffff);
}

#[gpui::test]
fn separate_palettes_and_system_mode_resolve_without_overwriting_choices(cx: &mut TestAppContext) {
    let mut state = cx.update(|cx| Appearance::new(cx));
    state.settings = Settings {
        mode: "system".into(),
        light_theme: "claude".into(),
        dark_theme: "graphite".into(),
        ..Settings::default()
    };
    let themes = imago_gpui::THEMES;
    assert_eq!(themes[state.resolve_dark(false).theme_index].key, "claude");
    assert_eq!(themes[state.resolve_dark(true).theme_index].key, "graphite");
    state.settings.mode = "light".into();
    assert!(!state.resolve_dark(true).dark);
    state.settings.mode = "dark".into();
    assert!(state.resolve_dark(false).dark);
}

#[gpui::test]
fn invalid_snapshots_preserve_style_and_missing_capabilities_fall_back(cx: &mut TestAppContext) {
    let mut state = cx.update(|cx| Appearance::new(cx));
    assert!(
        state.ingest(&json!({"settings":{"mode":"light","font_size":12.5,
        "material":"mica","font_family":"nonexistent-test-family","revision":7}}))
    );
    let good = state.settings.clone();
    for bad in [
        json!({}),
        json!({"settings":{"font_size":200}}),
        json!({"settings":{"schema_version":2}}),
    ] {
        assert!(!state.ingest(&bad));
        assert_eq!(state.settings, good);
    }
    let resolved = state.resolve_dark(false);
    assert_eq!(resolved.font_family, ".SystemUIFont");
    assert_eq!(resolved.material, "opaque");
    assert_eq!(resolved.font_size, 12.5);
}

#[gpui::test]
fn wallpaper_and_custom_accent_resolve_and_theme_restores_native_color(cx: &mut TestAppContext) {
    let mut state = cx.update(|cx| Appearance::new(cx));
    let mut settings = Settings {
        accent_source: "wallpaper".into(),
        ..Settings::default()
    };
    assert!(
        state.ingest(&json!({"settings":settings,"wallpaper_accent":"#1166AA",
        "capabilities":{"materials":["opaque"],"wallpaper_accent":true}}))
    );
    assert_eq!(state.resolve_dark(true).accent, Some(0x1166aa));
    settings.accent_source = "custom".into();
    settings.accent_color = Some("#FF0088".into());
    state.ingest(&json!({"settings":settings}));
    assert_eq!(state.resolve_dark(true).accent, Some(0xff0088));
    settings.accent_source = "theme".into();
    state.ingest(&json!({"settings":settings}));
    assert_eq!(state.resolve_dark(true).accent, None);
}

#[gpui::test]
fn appearance_is_a_static_separate_page_even_before_engine_values(cx: &mut TestAppContext) {
    let (manager, cx) = crate::a11y_tests::launch(cx);
    cx.simulate_resize(size(px(1440.), px(3000.)));
    for failed in [false, true] {
        manager.update(cx, |app, cx| {
            app.view = View::Appearance;
            app.slots.insert(
                "appearance".into(),
                if failed {
                    Slot::Failed("offline".into())
                } else {
                    Slot::Loading
                },
            );
            cx.notify();
        });
        cx.update(|_, cx| cx.refresh_windows());
        for selector in [
            "appearance-mode-system",
            "appearance-mode-light",
            "appearance-mode-dark",
        ] {
            assert!(
                cx.debug_bounds(selector).is_some(),
                "missing mode preview {selector}"
            );
        }
        for selector in [
            "appearance-accent-card",
            "appearance-apps-card",
            "appearance-material-card",
            "appearance-font-card",
        ] {
            assert!(
                cx.debug_bounds(selector).is_some(),
                "missing static card {selector}"
            );
        }
        let tree = crate::a11y_tests::a11y_tree(cx).to_string();
        assert!(
            !tree.contains("appearance-follow-apps"),
            "unknown policy must not look off"
        );
        assert!(cx.debug_bounds("settings-startup-card").is_none());
    }
}

#[gpui::test]
fn actual_typography_and_accent_follow_engine_not_just_root_font(cx: &mut TestAppContext) {
    let (manager, cx) = crate::a11y_tests::launch(cx);
    manager.update(cx, |app, cx| {
        app.slots.insert(
            "appearance".into(),
            Slot::Ready(json!({"settings":{
            "accent_source":"custom","accent_color":"#123ABC","font_size":18}})),
        );
        cx.notify();
    });
    cx.update(|_, cx| cx.refresh_windows());
    cx.update(|_, cx| {
        assert_eq!(crate::theme::ui_px(13.), px(18.));
        assert_eq!(crate::theme::ACCENT(), 0x123abc);
        assert_eq!(
            gpui_component::Theme::global(cx).colors.primary,
            crate::theme::c(0x123abc)
        );
    });
    manager.update(cx, |app, cx| {
        app.slots.insert(
            "appearance".into(),
            Slot::Ready(json!({"settings":Settings::default()})),
        );
        cx.notify();
    });
    cx.update(|_, cx| cx.refresh_windows());
    cx.update(|_, _| assert_eq!(crate::theme::ui_px(13.), px(13.)));
}
