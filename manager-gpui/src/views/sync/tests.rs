use super::{
    devices::{device_name, registry_devices},
    pairing_code, peer_status_text,
};
use crate::app::ManagerApp;
use gpui::{Entity, TestAppContext, VisualTestContext};
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

#[test]
fn registry_names_and_admission_state_override_discovery_metadata() {
    let rows = registry_devices(
        &json!({
            "localDeviceId": "this-device",
            "devices": [
                {"deviceId":"this-device", "displayName":"Work Mac", "enabled":true},
                {"deviceId":"phone", "displayName":"Reading phone", "enabled":true},
                {"deviceId":"old-laptop", "displayName":"Old laptop", "enabled":false},
            ]
        }),
        &json!({"running":true,"peers":[
            {"device_id":"phone", "device_name":"iPhone", "platform":"ios", "status":"online"},
            {"device_id":"old-laptop", "device_name":"MacBook", "status":"online"},
        ]}),
        "Host name",
    );

    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].name, "Work Mac");
    assert_eq!(rows[0].status, "Синхронизация активна");
    assert_eq!(rows[1].name, "Reading phone");
    assert_eq!(rows[1].status, "В сети");
    assert!(rows[1].has_peer);
    assert_eq!(rows[2].name, "Old laptop");
    assert_eq!(rows[2].status, "Отключено");
    assert!(!rows[2].enabled);
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
            "sync.registry".into(),
            crate::app::Slot::Ready(json!({
                "localDeviceId":"local-device",
                "devices":[
                    {"deviceId":"local-device", "enabled":true},
                    {"deviceId":"peer-id", "displayName":"Renamed Mac", "enabled":true},
                ]
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
    assert!(closed.contains("Отключить везде"));
    assert!(closed.contains("Забыть здесь"));
    assert!(closed.contains("Renamed Mac"));
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

/// KOS-369: a pending incoming pairing request renders the «Принять /
/// Отклонить» consent prompt with the requester's name and platform,
/// regardless of which view is active.
#[gpui::test]
fn pairing_consent_prompt_renders_requester(cx: &mut TestAppContext) {
    let (manager, cx) = crate::a11y_tests::launch(cx);
    manager.update(cx, |app, cx| {
        app.slots.insert(
            "sync.snapshot".into(),
            crate::app::Slot::Ready(json!({
                "running": true,
                "peers": [],
                "incoming_pairing_requests": [{
                    "device_id": "dev-pc",
                    "device_name": "Рабочий PC",
                    "platform": "windows",
                }],
            })),
        );
        cx.notify();
    });
    let tree = crate::a11y_tests::a11y_tree(cx).to_string();
    assert!(tree.contains("Рабочий PC"), "{tree}");
    assert!(tree.contains("Windows"), "{tree}");
    assert!(tree.contains("Принять"), "{tree}");
    assert!(tree.contains("Отклонить"), "{tree}");
}

/// KOS-369: after «Подключить» the initiator waits on the human — the card
/// shows «Ожидание подтверждения» with a «Отмена» action, and a declined
/// answer surfaces «Подключение отклонено» with the button usable again.
#[gpui::test]
fn pairing_card_shows_outgoing_states(cx: &mut TestAppContext) {
    let (manager, cx) = crate::a11y_tests::launch(cx);
    let set_snapshot =
        |manager: &Entity<ManagerApp>, cx: &mut VisualTestContext, outgoing: Value| {
            manager.update(cx, |app, cx| {
                app.view = crate::views::View::Sync;
                app.sync_pairing_open = true;
                app.slots.insert(
                    "sync.snapshot".into(),
                    crate::app::Slot::Ready(json!({
                        "running": true,
                        "peers": [],
                        "incoming_pairing_requests": [],
                        "outgoing_pairing": outgoing,
                    })),
                );
                cx.notify();
            });
        };

    set_snapshot(
        &manager,
        cx,
        json!({"status": "pending", "endpoint": "ep", "expires_in_ms": 60000}),
    );
    let tree = crate::a11y_tests::a11y_tree(cx).to_string();
    assert!(tree.contains("Ожидание подтверждения"), "{tree}");
    assert!(tree.contains("Отмена"), "{tree}");

    set_snapshot(
        &manager,
        cx,
        json!({"status": "declined", "endpoint": "ep"}),
    );
    let tree = crate::a11y_tests::a11y_tree(cx).to_string();
    assert!(tree.contains("Подключение отклонено"), "{tree}");
    assert!(!tree.contains("Ожидание подтверждения"), "{tree}");

    set_snapshot(
        &manager,
        cx,
        json!({"status": "pending", "endpoint": "ep", "expires_in_ms": 0}),
    );
    let tree = crate::a11y_tests::a11y_tree(cx).to_string();
    assert!(tree.contains("Не дождались подтверждения"), "{tree}");
}
