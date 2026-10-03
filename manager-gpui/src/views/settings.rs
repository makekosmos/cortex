//! User settings compose the existing controls without changing their operations.
use crate::app::{ManagerApp, Slot};
use crate::async_fields::field_text;
use crate::widgets::*;
use ::gpui::{prelude::*, *};
use gpui_component::Disableable;
use mundus_gpui_kit::theme::*;
use serde_json::json;

pub fn load(app: &mut ManagerApp) {
    app.call("engine.autostart", "engine.autostart.get", json!({}));
    super::engine_settings::load(app);
    super::browser::load(app);
    if app.settings_developer_open {
        super::dev::load(app);
    }
}

pub fn render(
    app: &mut ManagerApp,
    window: &mut Window,
    cx: &mut Context<ManagerApp>,
) -> AnyElement {
    let mut col = page_stack()
        .child(section(
            "Настройки",
            "Запуск, внешний вид, система и приватность",
        ))
        .child(section("Общие", ""));
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
    col = col
        .child(
            card()
                .id("settings-startup-card")
                .debug_selector(|| "settings-startup-card".into())
                .child(startup),
        )
        .child(section("Внешний вид", ""))
        .child(super::dev::render_appearance(app, window, cx))
        .child(super::engine_settings::render_body(app, window, cx))
        .child(super::browser::render_body(app, window, cx));
    col = col.child(
        card()
            .id("settings-developer-card")
            .debug_selector(|| "settings-developer-card".into())
            .child(
                row(
                    "Инструменты разработчика",
                    "Локальные пакеты, параметры инстанса и FPS.",
                )
                .child(
                    toggle(
                        "settings-developer",
                        app.settings_developer_open,
                        cx,
                        |this, open, _| {
                            this.settings_developer_open = open;
                            if open {
                                super::dev::load(this);
                            }
                        },
                    )
                    .accessibility_label("Инструменты разработчика")
                    .disabled(app.action_busy),
                ),
            ),
    );
    if app.settings_developer_open {
        col = col.child(super::dev::render_tools(app, window, cx));
    }
    col.into_any_element()
}
