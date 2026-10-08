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
fn downloading_shows_juicy_bar_instead_of_buttons(cx: &mut TestAppContext) {
    let (manager, cx) = crate::a11y_tests::launch(cx);
    set_status(
        cx,
        &manager,
        json!({
            "state": "downloading", "currentVersion": "0.10.2",
            "newVersion": "0.10.3", "percent": 42, "canInstall": true
        }),
    );
    // The bar replaces the whole button stack while a download runs.
    let track = cx.debug_bounds("update-overlay-progress").unwrap();
    assert!(cx.debug_bounds("update-overlay-primary").is_none());
    assert!(cx.debug_bounds("update-overlay-later").is_none());
    // After the eased fill settles it tracks percent: 42% of the track.
    manager.update(cx, |app, cx| {
        app.update_fill = 42.0;
        cx.notify();
    });
    cx.update(|_, cx| cx.refresh_windows());
    cx.run_until_parked();
    let fill = cx.debug_bounds("update-overlay-progress-fill").unwrap();
    let ratio = fill.size.width / track.size.width;
    assert!((ratio - 0.42).abs() < 0.05, "ratio {ratio}");
}

#[gpui::test]
fn offer_and_ready_keep_their_buttons(cx: &mut TestAppContext) {
    let (manager, cx) = crate::a11y_tests::launch(cx);
    set_status(
        cx,
        &manager,
        json!({
            "state": "available", "currentVersion": "0.10.2",
            "newVersion": "0.10.3", "canInstall": true
        }),
    );
    assert!(cx.debug_bounds("update-overlay-primary").is_some());
    assert!(cx.debug_bounds("update-overlay-later").is_some());
    set_status(
        cx,
        &manager,
        json!({
            "state": "downloaded", "currentVersion": "0.10.2",
            "newVersion": "0.10.3", "percent": 100, "canInstall": true
        }),
    );
    assert!(cx.debug_bounds("update-overlay-primary").is_some());
    assert!(cx.debug_bounds("update-overlay-later").is_some());
}

#[gpui::test]
fn hidden_window_pauses_logo_frames_until_visible(cx: &mut TestAppContext) {
    let (manager, cx) = crate::a11y_tests::launch(cx);
    set_status(
        cx,
        &manager,
        json!({
            "state": "available", "currentVersion": "0.10.2",
            "newVersion": "0.10.3", "canInstall": true
        }),
    );
    // Age the cached frame so the next visible render produces a fresh one.
    let stale = |manager: &gpui::Entity<crate::app::ManagerApp>,
                 cx: &mut gpui::VisualTestContext| {
        manager.update(cx, |app, cx| {
            if let Some((at, _)) = &mut app.update_logo_frame {
                *at = std::time::Instant::now() - std::time::Duration::from_secs(1);
            }
            cx.notify();
        });
    };
    stale(&manager, cx);
    cx.update(|_, cx| cx.refresh_windows());
    cx.run_until_parked();
    let produced = manager.read_with(cx, |app, _| app.update_logo_frames);
    assert!(produced > 0, "a visible overlay must produce logo frames");

    cx.simulate_visibility_change(gpui::WindowVisibility::Hidden);
    // The platform callback lands on the window on the next cycle; the
    // baseline is whatever that last still-visible render produced.
    stale(&manager, cx);
    cx.update(|_, cx| cx.refresh_windows());
    cx.run_until_parked();
    let produced_at_hide = manager.read_with(cx, |app, _| app.update_logo_frames);
    stale(&manager, cx);
    cx.update(|_, cx| cx.refresh_windows());
    cx.run_until_parked();
    cx.update(|_, cx| cx.refresh_windows());
    cx.run_until_parked();
    assert_eq!(
        manager.read_with(cx, |app, _| app.update_logo_frames),
        produced_at_hide,
        "hidden window must not produce logo frames"
    );

    cx.simulate_visibility_change(gpui::WindowVisibility::Visible);
    cx.update(|_, cx| cx.refresh_windows());
    cx.run_until_parked();
    stale(&manager, cx);
    cx.update(|_, cx| cx.refresh_windows());
    cx.run_until_parked();
    assert!(
        manager.read_with(cx, |app, _| app.update_logo_frames) > produced_at_hide,
        "visible window resumes logo frames"
    );
}

#[gpui::test]
fn downloading_then_error_offers_retry(cx: &mut TestAppContext) {
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
