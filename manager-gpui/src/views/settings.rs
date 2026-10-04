//! System controls; appearance has its own page and independent persistence.
use crate::app::{ManagerApp, Slot};
use crate::async_fields::field_text;
use crate::theme::*;
use crate::widgets::*;
use ::gpui::{prelude::*, *};
use serde_json::json;

pub fn load(app: &mut ManagerApp) {
    app.call("engine.autostart", "engine.autostart.get", json!({}));
    super::engine_settings::load(app);
}

pub fn render(
    app: &mut ManagerApp,
    _window: &mut Window,
    cx: &mut Context<ManagerApp>,
) -> AnyElement {
    let status = app.data("engine.autostart");
    let ready = matches!(app.slots.get("engine.autostart"), Some(Slot::Ready(_)));
    let mut startup = row(
        "Запуск при входе",
        "Запускается только Engine, без окна Manager.",
    );
    if ready && vbool(&status, "available") {
        startup = startup.child(
            toggle(
                "engine-autostart",
                vbool(&status, "enabled"),
                cx,
                |this, on, cx| {
                    this.action("engine.autostart.set", json!({"enabled":on}));
                    cx.notify();
                },
            )
            .accessibility_label("Автозапуск при входе"),
        );
    } else {
        startup = startup.child(badge(
            field_text(app.slots.get("engine.autostart"), |v| {
                vopt(v, "reason").unwrap_or_else(|| "Недоступно".into())
            }),
            MUTED_FG(),
        ));
    }
    let col = page_sections()
        .child(
            section_group()
                .id("settings-general-group")
                .debug_selector(|| "settings-general-group".into())
                .child(
                    section("Общие", "")
                        .id("settings-general-heading")
                        .debug_selector(|| "settings-general-heading".into()),
                )
                .child(
                    card()
                        .id("settings-startup-card")
                        .debug_selector(|| "settings-startup-card".into())
                        .child(startup),
                ),
        )
        .child(super::engine_settings::render_body(app, cx));
    col.into_any_element()
}
