//! Движок — engine.settings.get / engine.settings.set
//! (EngineSettingsView.vue parity: warm timeout + usage tracker).
use ::gpui::{prelude::*, *};
use gpui_component::input::Input;
use serde_json::json;

use crate::app::ManagerApp;
use crate::widgets::*;
use kosmos_gpui_kit::theme::*;

pub fn load(app: &mut ManagerApp) {
    app.call("engine.settings", "engine.settings.get", json!({}));
}

pub fn render(
    app: &mut ManagerApp,
    window: &mut Window,
    cx: &mut Context<ManagerApp>,
) -> AnyElement {
    let mut col = div().flex().flex_col().gap_4().w_full();
    col = col.child(section("Движок", "Настройки запуска"));

    col = col.child(slot_or(app, "engine.settings", |v| {
        let warm = vnum(vget(v, "desktop_host"), "warm_timeout_seconds");
        let tracker = vbool(vget(v, "usage_tracker"), "enabled");
        let mut el = card();
        el = el.child(
            div()
                .text_size(px(12.))
                .text_color(c(MUTED_FG()))
                .child("Текущие значения"),
        );
        el = el.child(kv("Тёплый таймаут", format!("{warm:.0} сек.")));
        el = el.child(kv(
            "Счётчик использования",
            if tracker {
                "Включён"
            } else {
                "Выключен"
            },
        ));
        el.into_any_element()
    }));

    let warm_in = app.input("engine.warm", "Таймаут в секундах…", window, cx);
    col = col.child(
        card()
            .child(
                div()
                    .text_size(px(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Тёплый таймаут"),
            )
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(c(MUTED_FG()))
                    .child("Через сколько секунд Engine выгружает неактивный хост."),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .items_center()
                    .child(div().w(px(200.)).child(Input::new(&warm_in)))
                    .child(btn(
                        "engine-warm-save",
                        "Сохранить",
                        true,
                        cx,
                        |this, cx| {
                            let raw = this.input_value("engine.warm", cx);
                            if let Ok(secs) = raw.parse::<u64>() {
                                this.action(
                                    "engine.settings.set",
                                    json!({"desktop_host": {"warm_timeout_seconds": secs}}),
                                );
                            } else {
                                this.error = Some("Введите число секунд.".into());
                            }
                        },
                    )),
            ),
    );

    let tracker_on = vbool(
        vget(&app.data("engine.settings"), "usage_tracker"),
        "enabled",
    );
    col = col.child(
        card().child(
            row(
                "Счётчик использования",
                "Анонимная статистика запусков для приоритизации разработки.",
            )
            .child(
                toggle("engine-tracker", tracker_on, cx, |this, checked, _| {
                    this.action(
                        "engine.settings.set",
                        json!({"usage_tracker": {"enabled": checked}}),
                    );
                })
                .accessibility_label("Счётчик использования"),
            ),
        ),
    );

    col.into_any_element()
}
