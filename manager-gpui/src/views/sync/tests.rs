use super::{device_name, pairing_code, peer_status_text};
use gpui::TestAppContext;
use serde_json::{json, Value};

#[test]
fn pairing_code_accepts_real_engine_string_and_legacy_objects() {
    assert_eq!(pairing_code(&json!(" actual-ticket ")), "actual-ticket");
    assert_eq!(pairing_code(&json!({"ticket":"ticket"})), "ticket");
    assert_eq!(pairing_code(&json!({"code":"legacy"})), "legacy");
    assert!(pairing_code(&Value::Null).is_empty());
}

#[test]
fn device_names_and_presence_have_safe_fallbacks() {
    assert_eq!(device_name(&json!({"device_name":"MacBook"})), "MacBook");
    assert_eq!(device_name(&json!({})), "Устройство");
    assert_eq!(peer_status_text("offline"), "Не в сети");
    assert_eq!(peer_status_text("online"), "В сети");
    assert_eq!(peer_status_text(""), "Статус неизвестен");
}

#[gpui::test]
fn pairing_form_is_hidden_until_opened(cx: &mut TestAppContext) {
    let (manager, cx) = crate::a11y_tests::launch(cx);
    manager.update(cx, |app, cx| {
        app.view = crate::views::View::Sync;
        app.slots.insert(
            "sync.snapshot".into(),
            crate::app::Slot::Ready(json!({
                "running":true, "transport":"iroh", "local_device":{"device_name":"This Mac"},
                "peers":[{"device_id":"peer-id", "device_name":"Other Mac", "status":"online"}]
            })),
        );
        app.slots.insert(
            "sync.ticket".into(),
            crate::app::Slot::Ready(json!("private-pairing-code")),
        );
        cx.notify();
    });
    cx.update(|_, cx| cx.refresh_windows());
    let closed = cx
        .update(|window, _| window.debug_a11y_tree_json())
        .unwrap();
    assert!(closed.contains("Подключить устройство"));
    assert!(closed.contains("Отключить"));
    assert!(!closed.contains("Скопировать код"));
    assert!(!closed.contains("private-pairing-code"));
    manager.update(cx, |app, cx| {
        app.sync_pairing_open = true;
        cx.notify();
    });
    cx.update(|_, cx| cx.refresh_windows());
    let opened = cx
        .update(|window, _| window.debug_a11y_tree_json())
        .unwrap();
    assert!(opened.contains("Скопировать код"));
    assert!(opened.contains("Отмена"));
    assert!(opened.contains("Подключить"));
    assert!(!opened.contains("private-pairing-code"));
}
