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
            app.view = View::About;
            app.slots.insert("upd.mundus".into(), slot);
            cx.notify();
        });
        cx.update(|_, cx| cx.refresh_windows());
        assert!(cx.debug_bounds("about-updates-card").is_some());
        assert!(cx.debug_bounds("about-updates-action").is_some());
    }
    assert_eq!(View::Sync.label(), "Девайсы");
}

#[gpui::test]
fn about_update_action_tracks_state_and_blocks_duplicate_requests(cx: &mut TestAppContext) {
    let (manager, cx) = crate::a11y_tests::launch(cx);
    cx.simulate_resize(gpui::size(gpui::px(900.), gpui::px(1200.)));
    for (state, can_install, enabled, label) in [
        ("idle", true, true, "Проверить обновления"),
        ("checking", true, false, "Проверяем…"),
        ("available", true, true, "Установить обновление"),
        ("available", false, false, "Установить обновление"),
        ("downloading", true, false, "Установить обновление"),
        ("downloaded", false, false, "Установить обновление"),
        ("downloaded", true, true, "Установить обновление"),
    ] {
        manager.update(cx, |app, cx| {
            app.view = View::About;
            app.action_busy = false;
            // The KOS-355 overlay otherwise covers the card on `available`.
            app.update_snoozed = true;
            app.slots.insert(
                "upd.mundus".into(),
                Slot::Ready(json!({
                    "state": state, "currentVersion": "0.10.2", "newVersion": "0.10.3",
                    "channel": "stable", "canInstall": can_install, "percent": 42
                })),
            );
            cx.notify();
        });
        cx.update(|_, cx| cx.refresh_windows());
        let action = cx.debug_bounds("about-updates-action").unwrap();
        let card = cx.debug_bounds("about-updates-card").unwrap();
        assert!(action.right() <= card.right());
        assert!(
            action.left() > card.center().x,
            "action must sit on the right"
        );
        let tree = cx
            .update(|window, _| window.debug_a11y_tree_json())
            .unwrap();
        assert!(tree.contains(label), "missing action for {state}");
        cx.simulate_click(action.center(), Default::default());
        manager.read_with(cx, |app, _| {
            assert_eq!(
                app.action_busy, enabled,
                "incorrect action availability for {state}"
            );
            if enabled {
                assert!(matches!(app.slots.get("@action"), Some(Slot::Loading)));
            }
        });
    }
}
