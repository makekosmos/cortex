use crate::app::Slot;
use crate::views::{about, View};
use gpui::TestAppContext;
use serde_json::json;

#[gpui::test]
fn static_about_fields_exist_before_engine_values_arrive(cx: &mut TestAppContext) {
    let (manager, cx) = crate::a11y_tests::launch(cx);
    manager.update(cx, |app, cx| {
        app.view = View::About;
        app.slots.insert("about.health".into(), Slot::Loading);
        app.slots.insert("about.catalog".into(), Slot::Loading);
        cx.notify();
    });
    cx.update(|_, cx| cx.refresh_windows());
    for selector in ["about-hero", "about-diagnostics-card", "about-updates-card"] {
        assert!(
            cx.debug_bounds(selector).is_some(),
            "missing static field {selector}"
        );
    }
    let before = cx.debug_bounds("about-diagnostics-card").unwrap();
    manager.update(cx, |app, cx| {
        app.slots.insert(
            "about.health".into(),
            Slot::Ready(json!({"status":"ready"})),
        );
        app.slots.insert(
            "about.catalog".into(),
            Slot::Ready(json!({"catalog":{"sequence":1}})),
        );
        cx.notify();
    });
    cx.update(|_, cx| cx.refresh_windows());
    assert_eq!(
        cx.debug_bounds("about-diagnostics-card").unwrap().size,
        before.size
    );
}

#[gpui::test]
fn opening_about_never_fetches_diagnostic_snapshot_or_logs(cx: &mut TestAppContext) {
    let (manager, cx) = crate::a11y_tests::launch(cx);
    manager.update(cx, |app, _| {
        about::load(app);
        assert!(app.slots.contains_key("upd.mundus"));
        assert!(app.slots.contains_key("about.health"));
        assert!(app.slots.contains_key("about.catalog"));
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
