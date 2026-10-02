//! Pairing + расширенные права cards for the Sync view (KOS-269): the
//! explicit LAN opt-ins and the one-time privilege grant that keeps the
//! firewall rule current across Engine updates.

use ::gpui::{prelude::*, *};
use serde_json::{json, Value};

use crate::app::ManagerApp;
use crate::widgets::*;
use mundus_gpui_kit::theme::*;

/// «Показать код для подключения» — the explicit LAN opt-in for the side
/// that shares its code (KOS-269): `show_pairing_code` escalates a
/// loopback-bound sync to LAN interfaces, which is when Windows may show
/// the firewall prompt on this device.
fn show_code_btn(cx: &mut Context<ManagerApp>) -> impl IntoElement {
    btn(
        "sync-show-code",
        "Показать код для подключения",
        true,
        cx,
        |this, _cx| {
            this.call("sync.ticket", "show_pairing_code", json!({}));
        },
    )
}

/// The `sync.ticket` slot value → displayed code. ark returns the ticket as
/// a *bare* JSON string (or `null`); the earlier `{"ticket"|"code"}`
/// object lookup survived from `get_own_iroh_ticket` days and rendered a
/// bare string as empty (KOS-269 round 2).
pub(super) fn pairing_code_from_slot(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Object(_) => {
            let ticket = vstr(v, "ticket");
            if ticket.is_empty() {
                vstr(v, "code")
            } else {
                ticket
            }
        }
        _ => String::new(),
    }
}

/// «Ваш код подключения» card. The ticket slot is populated only by the
/// explicit button — `sync.ticket` is absent until the user asks.
pub(super) fn ticket_card(app: &ManagerApp, cx: &mut Context<ManagerApp>) -> AnyElement {
    let slot = app.slot("sync.ticket");
    let mut el = card();
    el = el.child(
        div()
            .text_size(px(12.))
            .text_color(c(MUTED_FG()))
            .child("Ваш код подключения"),
    );
    let code = match slot {
        Some(Slot::Ready(v)) => pairing_code_from_slot(v),
        _ => String::new(),
    };
    match slot {
        Some(Slot::Loading) => {
            el = el.child(
                div()
                    .text_size(px(13.))
                    .text_color(c(MUTED_FG()))
                    .child("Загрузка…"),
            );
        }
        Some(Slot::Failed(e)) => {
            el = el
                .child(
                    div()
                        .text_size(px(13.))
                        .text_color(c(DESTRUCTIVE()))
                        .child(e.clone()),
                )
                .child(show_code_btn(cx));
        }
        _ if code.is_empty() => {
            el = el.child(show_code_btn(cx));
        }
        _ => {
            el = el.child(
                div()
                    .text_size(px(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(code),
            );
        }
    }
    el.into_any_element()
}

/// «Расширенные права» card: the LocalSystem service that keeps the
/// firewall rule current across Engine updates. `enable` triggers the one
/// UAC prompt; `status` (read on tab load) never elevates.
pub(super) fn privileged_card(app: &ManagerApp, cx: &mut Context<ManagerApp>) -> AnyElement {
    let mut el = card();
    el = el.child(
        div()
            .text_size(px(12.))
            .text_color(c(MUTED_FG()))
            .child("Расширенные права"),
    );
    match app.slot("sync.privileged") {
        Some(Slot::Loading) => {
            el = el.child(
                div()
                    .text_size(px(13.))
                    .text_color(c(MUTED_FG()))
                    .child("Загрузка…"),
            );
        }
        Some(Slot::Failed(e)) => {
            el = el.child(
                div()
                    .text_size(px(13.))
                    .text_color(c(DESTRUCTIVE()))
                    .child(e.clone()),
            );
        }
        Some(Slot::Ready(v)) => {
            let granted = vbool(v, "installed")
                && vbool(v, "running")
                && vbool(v, "pipe_ok")
                && vbool(v, "compatible");
            if granted {
                el = el.child(div().text_size(px(13.)).text_color(c(SUCCESS())).child(
                    "Включены — обновления приложения больше не будут \
                             показывать окно брандмауэра.",
                ));
            } else {
                el = el
                    .child(div().text_size(px(12.)).text_color(c(MUTED_FG())).child(
                        "Один раз позволяет службе следить за обновлениями — \
                                 окно брандмауэра после обновлений больше не появится.",
                    ))
                    .child(btn(
                        "sync-enable-privileged",
                        "Включить расширенные права (один раз)",
                        true,
                        cx,
                        |this, _cx| {
                            this.call("sync.privileged", "system.privileged.enable", json!({}));
                        },
                    ));
            }
        }
        None => {}
    }
    el.into_any_element()
}

#[cfg(test)]
mod tests {
    use super::pairing_code_from_slot;
    use serde_json::{json, Value};

    #[test]
    fn pairing_code_extracts_bare_string_and_object_forms() {
        // ark returns a bare ticket string — regression for the object-only
        // lookup that hid it behind the button forever.
        assert_eq!(
            pairing_code_from_slot(&json!("endpoint-abc")),
            "endpoint-abc"
        );
        assert_eq!(pairing_code_from_slot(&json!({"ticket": "t1"})), "t1");
        assert_eq!(pairing_code_from_slot(&json!({"code": "t2"})), "t2");
        assert_eq!(pairing_code_from_slot(&Value::Null), "");
        assert_eq!(pairing_code_from_slot(&json!({})), "");
    }
}
