use ::gpui::{prelude::*, *};
use serde_json::{json, Value};

use super::{block_row, device_avatar, device_identity, peer_status_text, presence};
use crate::app::{ManagerApp, Slot};
use crate::device_info::{peer_caption, peer_platform, Platform};
use crate::theme::*;
use crate::widgets::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct RegistryDeviceRow {
    pub(super) device_id: String,
    pub(super) name: String,
    pub(super) detail: String,
    pub(super) status: String,
    pub(super) enabled: bool,
    pub(super) is_local: bool,
    pub(super) has_peer: bool,
    pub(super) platform: Platform,
}

pub(super) fn local_name(registry: &Value, local_id: &str, system_name: &str) -> String {
    varr(registry, "devices")
        .iter()
        .find(|device| vstr(device, "deviceId") == local_id)
        .and_then(|device| vopt(device, "displayName"))
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| system_name.to_owned())
}

pub(super) fn local_status(registry: &Value, sync: &Value) -> (&'static str, u32) {
    let local_id = vstr(registry, "localDeviceId");
    let enabled = varr(registry, "devices")
        .iter()
        .find(|device| vstr(device, "deviceId") == local_id)
        .map(|device| vbool(device, "enabled"))
        .unwrap_or(true);
    if !enabled {
        ("Устройство отключено", WARN())
    } else if vbool(sync, "running") {
        ("Синхронизация активна", SUCCESS())
    } else {
        ("Синхронизация остановлена", WARN())
    }
}

pub(super) fn render_local_name_editor(
    app: &mut ManagerApp,
    local_id: &str,
    current_name: &str,
    window: &mut Window,
    cx: &mut Context<ManagerApp>,
) -> Div {
    let key = registry_name_key(local_id);
    name_editor(app, local_id, &key, current_name.to_owned(), window, cx)
}

pub(super) fn render_others(
    app: &mut ManagerApp,
    registry: &Value,
    sync: &Value,
    system_name: &str,
    window: &mut Window,
    cx: &mut Context<ManagerApp>,
) -> Div {
    let ready = matches!(app.slots.get("sync.registry"), Some(Slot::Ready(_)));
    let mut rows = div().flex().flex_col();
    let mut count = 0;
    for device in registry_devices(registry, sync, system_name)
        .into_iter()
        .filter(|device| !device.is_local)
    {
        rows = rows.child(render_device(app, device, count == 0, window, cx));
        count += 1;
    }
    if count == 0 {
        rows = rows.child(
            block_row(true).child(div().text_color(c(MUTED_FG())).child(if ready {
                "Других устройств пока нет. Подключите их по коду ниже."
            } else {
                "Загружаем список устройств…"
            })),
        );
    }
    rows
}

pub(super) fn registry_devices(
    registry: &Value,
    sync: &Value,
    local_name: &str,
) -> Vec<RegistryDeviceRow> {
    let local_id = vstr(registry, "localDeviceId");
    let peers = varr(sync, "peers");
    varr(registry, "devices")
        .iter()
        .map(|device| {
            let device_id = vstr(device, "deviceId");
            let is_local = !local_id.is_empty() && device_id == local_id;
            let peer = peers
                .iter()
                .find(|peer| vstr(peer, "device_id") == device_id);
            let enabled = vbool(device, "enabled");
            let name = vopt(device, "displayName")
                .filter(|name| !name.trim().is_empty())
                .or_else(|| is_local.then(|| local_name.to_owned()))
                .or_else(|| peer.map(device_name))
                .unwrap_or_else(|| "Устройство".into());
            let status = if !enabled {
                "Отключено"
            } else if is_local {
                if vbool(sync, "running") {
                    "Синхронизация активна"
                } else {
                    "Синхронизация остановлена"
                }
            } else {
                match peer.map(|peer| vstr(peer, "status")) {
                    Some(status) => peer_status_text(&status),
                    None => "Не подключено",
                }
            }
            .to_owned();
            let detail = peer
                .map(peer_caption)
                .filter(|detail| !detail.trim().is_empty())
                .unwrap_or_else(|| format!("ID: {device_id}"));
            RegistryDeviceRow {
                device_id,
                name,
                detail,
                status,
                enabled,
                is_local,
                has_peer: peer.is_some(),
                platform: peer.map(peer_platform).unwrap_or(Platform::Unknown),
            }
        })
        .collect()
}

pub(super) fn device_name(device: &Value) -> String {
    vopt(device, "device_name")
        .or_else(|| vopt(device, "name"))
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "Устройство".into())
}

fn registry_name_key(device_id: &str) -> String {
    format!("sync.registry.name.{device_id}")
}

fn name_editor(
    app: &mut ManagerApp,
    device_id: &str,
    input_key: &str,
    current_name: String,
    window: &mut Window,
    cx: &mut Context<ManagerApp>,
) -> Div {
    let input = app.input(
        input_key,
        format!("Новое имя для {current_name}"),
        false,
        window,
        cx,
    );
    let key = input_key.to_owned();
    let id = device_id.to_owned();
    div()
        .w_full()
        .flex()
        .flex_wrap()
        .gap_2()
        .child(
            div()
                .flex_1()
                .min_w(px(180.))
                .child(input_field(&input).aria_label(format!("Новое имя для {current_name}"))),
        )
        .child(
            crate::button::ghost(SharedString::from(format!("rename-{id}")))
                .label("Сохранить имя")
                .disabled(app.action_busy || device_id.is_empty())
                .on_click(cx.listener(move |this, _, _, cx| {
                    let display_name = this.input_value(&key, cx);
                    if display_name.is_empty() {
                        return;
                    }
                    this.action(
                        "device_registry.rename",
                        json!({
                            "target_device_id": id,
                            "display_name": display_name,
                        }),
                    );
                    cx.notify();
                })),
        )
}

fn render_device(
    app: &mut ManagerApp,
    device: RegistryDeviceRow,
    first: bool,
    window: &mut Window,
    cx: &mut Context<ManagerApp>,
) -> Div {
    let name = device.name.clone();
    let id = device.device_id.clone();
    let mut row = block_row(first)
        .flex_col()
        .items_start()
        .child(
            div()
                .w_full()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_3()
                .child(device_avatar(device.platform))
                .child(device_identity(&device.name, &device.detail))
                .child(presence(
                    &device.status,
                    if device.status == "В сети" {
                        SUCCESS()
                    } else if !device.enabled {
                        WARN()
                    } else {
                        MUTED_FG()
                    },
                )),
        )
        .child(name_editor(
            app,
            &id,
            &registry_name_key(&id),
            name.clone(),
            window,
            cx,
        ));

    if device.enabled {
        let disable_id = id.clone();
        let confirmation_name = name.clone();
        row = row.child(
            crate::button::ghost(SharedString::from(format!("disable-{id}")))
                .label("Отключить везде")
                .disabled(app.action_busy)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.ask_confirm(
                        "Отключить устройство везде",
                        format!(
                            "Устройство «{confirmation_name}» больше не сможет синхронизироваться. "
                        ) + "Уже сохранённые на нём данные не удалятся.",
                        "device_registry.disable",
                        json!({"target_device_id": disable_id}),
                        cx,
                    );
                })),
        );
    }
    if device.has_peer {
        let forget_id = id.clone();
        row = row.child(
            crate::button::ghost(SharedString::from(format!("forget-{id}")))
                .label("Забыть здесь")
                .disabled(app.action_busy)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.ask_confirm(
                        "Убрать устройство только здесь",
                        format!("«{name}» будет удалено из локального списка подключений. ")
                            + "Остальные устройства не отключатся.",
                        "disconnect_peer",
                        json!({"device_id": forget_id}),
                        cx,
                    );
                })),
        );
    }
    row
}
