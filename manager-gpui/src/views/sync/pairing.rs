use ::gpui::{prelude::*, *};
use serde_json::{json, Value};

use super::{block, block_row, device_identity, pairing_code, presence, section_label};
use crate::app::ManagerApp;
use crate::widgets::*;
use mundus_gpui_kit::theme::*;

pub(super) fn render(
    app: &mut ManagerApp,
    window: &mut Window,
    cx: &mut Context<ManagerApp>,
) -> AnyElement {
    let open = app.sync_pairing_open;
    let mut content = block().child(
        block_row(true)
            .child(device_identity(
                "Подключить ещё одно устройство",
                "Скопируйте свой код или введите код другого устройства.",
            ))
            .child(
                btn(
                    "sync-pairing-toggle",
                    if open {
                        "Отмена"
                    } else {
                        "Подключить устройство"
                    },
                    false,
                    cx,
                    |app, cx| {
                        app.sync_pairing_open = !app.sync_pairing_open;
                        app.sync_code_copied = false;
                        if app.sync_pairing_open {
                            app.refresh("sync.ticket", "show_pairing_code", json!({}));
                        }
                        cx.notify();
                    },
                )
                .disabled(app.action_busy),
            ),
    );
    if open {
        let code = pairing_code(&app.data("sync.ticket"));
        let can_copy = !code.is_empty();
        let copied = app.sync_code_copied;
        content = content.child(
            block_row(false)
                .child(device_identity(
                    "Код этого устройства",
                    if can_copy {
                        "Передайте его на другое устройство для подключения."
                    } else {
                        "Код пока недоступен. Дождитесь готовности синхронизации."
                    },
                ))
                .child(
                    btn(
                        "sync-copy-code",
                        if copied {
                            "Скопировано"
                        } else {
                            "Скопировать код"
                        },
                        false,
                        cx,
                        move |app, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(code.clone()));
                            app.sync_code_copied = true;
                            cx.notify();
                        },
                    )
                    .disabled(!can_copy),
                ),
        );
        let input = app.input("sync.peer", "Код другого устройства…", false, window, cx);
        content = content.child(
            block_row(false)
                .flex_col()
                .items_start()
                .gap_3()
                .child(section_label("Подключение по коду"))
                .child(
                    div()
                        .w_full()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(div().flex_1().min_w_0().child(input_field(&input)))
                        .child(
                            btn(
                                "sync-connect",
                                if app.action_busy {
                                    "Подключение…"
                                } else {
                                    "Подключить"
                                },
                                true,
                                cx,
                                |app, cx| {
                                    let code = app.input_value("sync.peer", cx);
                                    if !code.is_empty() {
                                        app.action(
                                            "connect_with_pairing_code",
                                            json!({"ticket": code, "pairing_code": code}),
                                        );
                                        cx.notify();
                                    }
                                },
                            )
                            .disabled(app.action_busy),
                        ),
                ),
        );
        // KOS-369: the connect RPC returns `pending` at once — the outcome
        // arrives through `outgoing_pairing` in the snapshot. While pending
        // the row offers «Отмена»; a terminal state shows the result and a
        // «Скрыть» that clears it engine-side.
        let outgoing = app.data("sync.snapshot");
        let outgoing = outgoing.get("outgoing_pairing");
        let status = outgoing
            .and_then(|o| o.get("status"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        let pending = status == "pending";
        let timed_out = pending
            && outgoing
                .and_then(|o| o.get("expires_in_ms"))
                .and_then(Value::as_u64)
                == Some(0);
        let (line, terminal) = match status {
            _ if timed_out => (
                "Не дождались подтверждения — проверьте код и повторите.",
                true,
            ),
            "pending" => ("Ожидание подтверждения на другом устройстве…", false),
            "declined" => ("Подключение отклонено.", true),
            "connected" => ("Устройство подключено.", true),
            _ => ("", false),
        };
        if !line.is_empty() {
            // The status text lives in the button's accessible name: the a11y
            // tree (and assistive tech) only expose interactive nodes.
            content = content.child(
                block_row(false).child(device_identity(line, "")).child(
                    btn(
                        if terminal {
                            "sync-pairing-dismiss"
                        } else {
                            "sync-pairing-cancel"
                        },
                        if terminal {
                            "Скрыть"
                        } else {
                            "Отмена"
                        },
                        false,
                        cx,
                        |app, cx| {
                            app.action("cancel_pairing", json!({}));
                            cx.notify();
                        },
                    )
                    .accessibility_label(line)
                    .disabled(app.action_busy),
                ),
            );
        }
    }
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(section_label("Добавить устройство"))
        .child(content)
        .child(privileged_row(app, cx))
        .into_any_element()
}

/// KOS-269 «Расширенные права»: the LocalSystem service that keeps the
/// firewall rule current across Engine updates. `enable` triggers the one
/// UAC prompt; `status` (read on tab load) never elevates.
fn privileged_row(app: &ManagerApp, cx: &mut Context<ManagerApp>) -> AnyElement {
    let status = app.data("sync.privileged");
    let granted = vbool(&status, "installed")
        && vbool(&status, "running")
        && vbool(&status, "pipe_ok")
        && vbool(&status, "compatible");
    block()
        .child(
            block_row(true)
                .child(device_identity(
                    "Расширенные права",
                    if granted {
                        "Включены — обновления приложения больше не будут \
                     показывать окно брандмауэра."
                    } else {
                        "Один раз позволяет службе следить за обновлениями — окно \
                     брандмауэра после обновлений больше не появится."
                    },
                ))
                .child(if granted {
                    presence("Включены", SUCCESS()).into_any_element()
                } else {
                    btn(
                        "sync-enable-privileged",
                        "Включить",
                        true,
                        cx,
                        |this, _cx| {
                            this.call("sync.privileged", "system.privileged.enable", json!({}));
                        },
                    )
                    .into_any_element()
                }),
        )
        .into_any_element()
}
