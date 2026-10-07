use super::{resolve, OverlayState};
use crate::app::Slot;
use gpui::TestAppContext;
use serde_json::json;

// --- State machine -----------------------------------------------------------

#[test]
fn engine_states_map_to_overlay_states() {
    assert_eq!(resolve(false, false, "idle"), OverlayState::Hidden);
    assert_eq!(resolve(false, false, "checking"), OverlayState::Hidden);
    assert_eq!(resolve(false, false, "not-available"), OverlayState::Hidden);
    assert_eq!(resolve(false, false, "available"), OverlayState::Offer);
    assert_eq!(
        resolve(false, false, "downloading"),
        OverlayState::Downloading
    );
    assert_eq!(resolve(false, false, "downloaded"), OverlayState::Ready);
}

#[test]
fn snooze_hides_every_state_for_the_session() {
    for engine in ["available", "downloading", "downloaded", "error"] {
        assert_eq!(
            resolve(true, true, engine),
            OverlayState::Hidden,
            "{engine}"
        );
    }
}

#[test]
fn error_only_shows_when_overlay_was_open() {
    // A background check failure must not hijack the window.
    assert_eq!(resolve(false, false, "error"), OverlayState::Hidden);
    assert_eq!(resolve(false, true, "error"), OverlayState::Failed);
}

#[test]
fn full_flow_offer_to_ready_to_hidden() {
    let mut state = OverlayState::Hidden;
    state = resolve(false, state.visible(), "available");
    assert_eq!(state, OverlayState::Offer);
    state = resolve(false, state.visible(), "downloading");
    assert_eq!(state, OverlayState::Downloading);
    state = resolve(false, state.visible(), "downloaded");
    assert_eq!(state, OverlayState::Ready);
    state = resolve(false, state.visible(), "idle");
    assert_eq!(state, OverlayState::Hidden);
}

// --- Rendered overlay ----------------------------------------------------------

fn set_status(
    cx: &mut gpui::VisualTestContext,
    manager: &gpui::Entity<crate::app::ManagerApp>,
    status: serde_json::Value,
) {
    manager.update(cx, |app, cx| {
        app.slots.insert("upd.mundus".into(), Slot::Ready(status));
        cx.notify();
    });
    cx.update(|_, cx| cx.refresh_windows());
}

#[gpui::test]
fn overlay_appears_on_available_and_snoozes_for_the_session(cx: &mut TestAppContext) {
    let (manager, cx) = crate::a11y_tests::launch(cx);
    set_status(
        cx,
        &manager,
        json!({
            "state": "available", "currentVersion": "0.10.2",
            "newVersion": "0.10.3", "canInstall": true
        }),
    );
    assert!(cx.debug_bounds("update-overlay").is_some());
    let tree = cx
        .update(|window, _| window.debug_a11y_tree_json())
        .unwrap();
    for name in ["Обновить", "Позже"] {
        assert!(tree.contains(name), "missing {name}: {tree}");
    }
    // «Позже» snoozes — the overlay disappears and stays gone.
    let later = cx.debug_bounds("update-overlay-later").unwrap();
    cx.simulate_click(later.center(), Default::default());
    cx.update(|_, cx| cx.refresh_windows());
    assert!(cx.debug_bounds("update-overlay").is_none());
    manager.read_with(cx, |app, _| assert!(app.update_snoozed));
    set_status(
        cx,
        &manager,
        json!({
            "state": "downloaded", "currentVersion": "0.10.2",
            "newVersion": "0.10.3", "percent": 100, "canInstall": true
        }),
    );
    assert!(cx.debug_bounds("update-overlay").is_none());
}

#[gpui::test]
fn downloading_shows_determinate_bar_and_error_offers_retry(cx: &mut TestAppContext) {
    let (manager, cx) = crate::a11y_tests::launch(cx);
    set_status(
        cx,
        &manager,
        json!({
            "state": "downloading", "currentVersion": "0.10.2",
            "newVersion": "0.10.3", "percent": 42, "canInstall": true
        }),
    );
    assert!(cx.debug_bounds("update-overlay-progress").is_some());
    // Failure while open → error state with a retry button.
    set_status(
        cx,
        &manager,
        json!({"state": "error", "currentVersion": "0.10.2", "message": "network down"}),
    );
    let tree = cx
        .update(|window, _| window.debug_a11y_tree_json())
        .unwrap();
    assert!(tree.contains("Повторить"), "{tree}");
    assert!(cx.debug_bounds("update-overlay").is_some());
    let retry = cx.debug_bounds("update-overlay-primary").unwrap();
    cx.simulate_click(retry.center(), Default::default());
    manager.read_with(cx, |app, _| {
        assert!(matches!(app.slots.get("@action"), Some(Slot::Loading)));
    });
}

#[gpui::test]
fn logo_stays_put_while_metal_flows(cx: &mut TestAppContext) {
    let (manager, cx) = crate::a11y_tests::launch(cx);
    set_status(
        cx,
        &manager,
        json!({
            "state": "available", "currentVersion": "0.10.2",
            "newVersion": "0.10.3", "canInstall": true
        }),
    );
    let first = cx.debug_bounds("update-overlay-logo").unwrap();
    // Advance the animation clock well past a full metal-flow cycle; the
    // shader keeps animating but the mark must not move.
    manager.update(cx, |app, cx| {
        app.update_anim_start = std::time::Instant::now() - std::time::Duration::from_millis(1950);
        cx.notify();
    });
    cx.update(|_, cx| cx.refresh_windows());
    let second = cx.debug_bounds("update-overlay-logo").unwrap();
    assert_eq!(first, second);
}

#[gpui::test]
fn background_error_without_open_overlay_stays_hidden(cx: &mut TestAppContext) {
    let (manager, cx) = crate::a11y_tests::launch(cx);
    set_status(
        cx,
        &manager,
        json!({"state": "error", "currentVersion": "0.10.2", "message": "feed unreachable"}),
    );
    assert!(cx.debug_bounds("update-overlay").is_none());
}
