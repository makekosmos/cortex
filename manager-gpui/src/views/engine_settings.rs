//! Движок — engine.settings.get / engine.settings.set
//! (EngineSettingsView.vue parity: usage tracker).
use ::gpui::{prelude::*, *};
use gpui_component::Disableable;
use serde_json::json;

use crate::app::ManagerApp;
use crate::async_fields::field_row;
use crate::theme::*;
use crate::widgets::*;

pub fn load(app: &mut ManagerApp) {
    app.call("engine.settings", "engine.settings.get", json!({}));
}

pub fn render_body(app: &mut ManagerApp, cx: &mut Context<ManagerApp>) -> AnyElement {
    let mut col = section_group()
        .id("settings-system-group")
        .debug_selector(|| "settings-system-group".into())
        .child(section("Система и производительность", "Параметры Engine"));

    col = col.child(
        card()
            .child(
                div()
                    .text_size(crate::theme::ui_px(12.))
                    .text_color(c(MUTED_FG()))
                    .child("Текущие значения"),
            )
            .child(field_row(
                app,
                "engine.settings",
                "Счётчик использования",
                |v| {
                    if vbool(vget(v, "usage_tracker"), "enabled") {
                        "Включён".into()
                    } else {
                        "Выключен".into()
                    }
                },
            )),
    );

    let tracker_on = vbool(
        vget(&app.data("engine.settings"), "usage_tracker"),
        "enabled",
    );
    col = col.child(
        card().child(
            row(
                "Счётчик использования",
                "Учитывать активность приложений. Настройка сохраняется в Engine.",
            )
            .child(
                toggle("engine-tracker", tracker_on, cx, |this, checked, _| {
                    this.action(
                        "engine.settings.set",
                        json!({"usage_tracker": {"enabled": checked}}),
                    );
                })
                .accessibility_label("Счётчик использования")
                .disabled(!matches!(
                    app.slots.get("engine.settings"),
                    Some(crate::app::Slot::Ready(_))
                )),
            ),
        ),
    );

    col.into_any_element()
}
