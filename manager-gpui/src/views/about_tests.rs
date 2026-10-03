use crate::app::Slot;
use crate::views::{about, View};
use gpui::TestAppContext;
use serde_json::json;

#[gpui::test]
fn static_about_fields_exist_before_engine_values_arrive(cx: &mut TestAppContext) {
    let (manager, cx) = crate::a11y_tests::launch(cx);
    manager.update(cx, |app, cx| {
        app.view = View::About;
        app.slots.insert("about.info".into(), Slot::Loading);
        app.slots.insert("about.health".into(), Slot::Loading);
        cx.notify();
    });
    cx.update(|_, cx| cx.refresh_windows());
    for selector in [
        "about-product-card",
        "about-engine-card",
        "about-field-version",
        "about-field-api_version",
        "about-field-build",
        "about-field-channel",
    ] {
        assert!(
            cx.debug_bounds(selector).is_some(),
            "missing static field {selector}"
        );
    }
    let before = cx.debug_bounds("about-engine-card").unwrap();
    manager.update(cx, |app, cx| {
        app.slots.insert("about.info".into(), Slot::Ready(json!({"version":"1.2.3", "api_version":"1.0.0", "build":"abc123", "channel":"stable"})));
        cx.notify();
    });
    cx.update(|_, cx| cx.refresh_windows());
    assert_eq!(
        cx.debug_bounds("about-engine-card").unwrap().size,
        before.size
    );
    manager.update(cx, |app, cx| {
        app.slots
            .insert("about.info".into(), Slot::Failed("offline".into()));
        cx.notify();
    });
    cx.update(|_, cx| cx.refresh_windows());
    assert!(cx.debug_bounds("about-field-api_version").is_some());
}

#[gpui::test]
fn opening_about_never_fetches_diagnostic_snapshot_or_logs(cx: &mut TestAppContext) {
    let (manager, cx) = crate::a11y_tests::launch(cx);
    manager.update(cx, |app, _| {
        about::load(app);
        assert!(app.slots.contains_key("about.info"));
        assert!(app.slots.contains_key("about.health"));
        assert!(!app.slots.contains_key("about.diag"));
        assert!(!app.slots.contains_key("about.logs"));
        assert!(!app.slots.contains_key("@bundle"));
    });
}

#[gpui::test]
fn support_tools_are_opt_in_not_regular_about_content(cx: &mut TestAppContext) {
    let (manager, cx) = crate::a11y_tests::launch(cx);
    manager.update(cx, |app, cx| {
        app.view = View::About;
        cx.notify();
    });
    cx.update(|_, cx| cx.refresh_windows());
    let hidden = cx
        .update(|window, _| window.debug_a11y_tree_json())
        .unwrap();
    assert!(!hidden.contains("Создать пакет поддержки"));
    assert!(!hidden.contains("Открыть папку журналов"));
    manager.update(cx, |app, cx| {
        app.about_support_open = true;
        cx.notify();
    });
    cx.update(|_, cx| cx.refresh_windows());
    let shown = cx
        .update(|window, _| window.debug_a11y_tree_json())
        .unwrap();
    assert!(shown.contains("Создать пакет поддержки"));
    assert!(shown.contains("Открыть папку журналов"));
}
