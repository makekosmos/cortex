use crate::app::Slot;
use crate::device_info::version_label;
use crate::views::View;
use gpui::TestAppContext;
use serde_json::json;

#[test]
fn version_labels_follow_engine_metadata_without_replacing_current_with_available() {
    let status = json!({"currentVersion":"0.10.2", "channel":"stable", "newVersion":"0.10.3"});
    assert_eq!(
        version_label(
            status["currentVersion"].as_str().unwrap(),
            status["channel"].as_str().unwrap()
        ),
        "0.10.2"
    );
    assert_eq!(version_label("0.1.0", "dev"), "0.1.0 (dev)");
    assert_eq!(version_label("0.10.3", "stable"), "0.10.3");
    assert_eq!(version_label("", "dev"), "");
}

#[gpui::test]
fn product_card_and_action_exist_while_metadata_loads_or_fails(cx: &mut TestAppContext) {
    let (manager, cx) = crate::a11y_tests::launch(cx);
    for slot in [
        Slot::Loading,
        Slot::Ready(json!({
            "state": "idle", "currentVersion": "0.10.3", "channel": "stable"
        })),
        Slot::Failed("offline".into()),
    ] {
        manager.update(cx, |app, cx| {
            app.view = View::Updates;
            app.slots.insert("upd.mundus".into(), slot);
            cx.notify();
        });
        cx.update(|_, cx| cx.refresh_windows());
        assert!(cx.debug_bounds("updates-product-card").is_some());
        assert!(cx.debug_bounds("manager-button-caption").is_some());
    }
    assert_eq!(View::Sync.label(), "Девайсы");
}
