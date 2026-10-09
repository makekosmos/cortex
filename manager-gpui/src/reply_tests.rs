//! KOS-367: pairing reply surfacing — the «Подключить» result must reach
//! the banner/notice and re-enable the button.
use gpui::TestAppContext;
use serde_json::json;

use crate::a11y_tests::launch;
/// KOS-367: «Подключить» must never fail silently — an Engine error lands in
/// the banner and re-enables the button, and a connected reply names the
/// peer in the notice.
#[gpui::test]
async fn pairing_connect_reply_surfaces_outcome(cx: &mut TestAppContext) {
    let (manager, cx) = launch(cx);
    let params = json!({"ticket": "endpointx", "pairing_code": "endpointx"});

    manager.update(cx, |app, _cx| {
        app.action("connect_with_pairing_code", params.clone());
    });
    manager.read_with(cx, |app, _| assert!(app.action_busy));

    manager.update(cx, |app, _cx| {
        app.action_reply(Err("device did not respond".into()));
    });
    manager.read_with(cx, |app, _| {
        assert!(!app.action_busy);
        assert_eq!(app.error.as_deref(), Some("device did not respond"));
        assert!(app.notice.is_none());
    });

    manager.update(cx, |app, _cx| {
        app.action("connect_with_pairing_code", params);
        app.action_reply(Ok(json!({
            "status": "connected",
            "device_id": "dev-mac",
            "device_name": "MacBook",
        })));
    });
    manager.read_with(cx, |app, _| {
        assert!(!app.action_busy);
        assert_eq!(
            app.notice.as_deref(),
            Some("Устройство подключено: MacBook.")
        );
    });

    // Other actions keep the generic line.
    manager.update(cx, |app, _cx| {
        app.action_reply(Ok(json!(true)));
    });
    manager.read_with(cx, |app, _| {
        assert_eq!(app.notice.as_deref(), Some("Выполнено."));
    });
}
