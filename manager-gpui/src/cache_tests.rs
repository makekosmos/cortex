use super::{Slot, View};
use crate::a11y_tests::launch;
use gpui::TestAppContext;
use serde_json::json;

#[gpui::test]
fn revisiting_page_keeps_cached_data_visible(cx: &mut TestAppContext) {
    let (manager, cx) = launch(cx);
    manager.update(cx, |app, cx| {
        for slot in ["data.summary", "data.types", "data.storage"] {
            app.slots
                .insert(slot.into(), Slot::Ready(json!({"cached": true})));
        }
        app.set_view(View::Engine, cx);
        app.set_view(View::Data, cx);
        for slot in ["data.summary", "data.types", "data.storage"] {
            assert!(matches!(app.slots.get(slot), Some(Slot::Ready(v)) if v["cached"] == true));
        }
    });
}

#[gpui::test]
fn explicit_refresh_and_invalidated_cache_still_reload(cx: &mut TestAppContext) {
    let (manager, cx) = launch(cx);
    manager.update(cx, |app, cx| {
        app.slots
            .insert("engine.settings".into(), Slot::Ready(json!({})));
        app.set_view(View::Engine, cx);
        assert!(matches!(
            app.slots.get("engine.settings"),
            Some(Slot::Ready(_))
        ));
        app.load_current();
        assert!(matches!(
            app.slots.get("engine.settings"),
            Some(Slot::Loading)
        ));
        app.slots
            .insert("engine.settings".into(), Slot::Ready(json!({})));
        app.invalidated_slots.insert("engine.settings".into());
        app.set_view(View::Data, cx);
        app.set_view(View::Engine, cx);
        assert!(matches!(
            app.slots.get("engine.settings"),
            Some(Slot::Loading)
        ));
    });
}

#[gpui::test]
fn navigation_reuses_pending_slots_but_retries_failed_slots(cx: &mut TestAppContext) {
    let (manager, cx) = launch(cx);
    manager.update(cx, |app, _| {
        app.navigation_load = true;
        app.slots.insert("pending".into(), Slot::Loading);
        app.slots
            .insert("failed".into(), Slot::Failed("offline".into()));
        assert!(app.reuse_navigation_slot("pending"));
        assert!(!app.reuse_navigation_slot("failed"));
        assert!(!app.reuse_navigation_slot("missing"));
    });
}

#[gpui::test]
fn navigation_cache_applies_to_health_and_usage_requests(cx: &mut TestAppContext) {
    let (manager, cx) = launch(cx);
    manager.update(cx, |app, _| {
        app.navigation_load = true;
        app.slots.insert(
            "about.health".into(),
            Slot::Ready(json!({"status":"ready"})),
        );
        app.slots
            .insert("usage.report".into(), Slot::Ready(json!({"topApps":[]})));
        app.status("about.health", "health");
        app.usage_report("usage.report");
        assert!(matches!(
            app.slots.get("about.health"),
            Some(Slot::Ready(_))
        ));
        assert!(matches!(
            app.slots.get("usage.report"),
            Some(Slot::Ready(_))
        ));
    });
}
