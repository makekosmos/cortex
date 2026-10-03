//! Ключи — dictation.get_config/verify_api_key/set_api_key/clear_api_key/
//! test_connectivity + stats (SecretsView.vue parity).
use ::gpui::{prelude::*, *};
use serde_json::json;

use crate::app::ManagerApp;
use crate::widgets::*;
use mundus_gpui_kit::theme::*;

pub fn load(app: &mut ManagerApp) {
    app.call("secrets.config", "dictation.get_config", json!({}));
    app.call("secrets.stats", "dictation.get_stats", json!({}));
}

pub fn render_body(
    app: &mut ManagerApp,
    window: &mut Window,
    cx: &mut Context<ManagerApp>,
) -> AnyElement {
    let mut col = page_stack().child(section(
        "Доступы и API-ключи",
        "Ключи сгруппированы по назначению",
    ));
    let key_status = crate::async_fields::field_text(app.slots.get("secrets.config"), |v| {
        let global = v.get("hasApiKey").and_then(serde_json::Value::as_bool);
        let nested = vget(v, "config")
            .get("hasApiKey")
            .and_then(serde_json::Value::as_bool);
        match (global, nested) {
            (Some(true), _) | (_, Some(true)) => "Ключ задан".into(),
            (Some(false), _) | (_, Some(false)) => "Ключ не задан".into(),
            _ => String::new(),
        }
    });
    let mut config = card()
        .id("integration-access-card")
        .debug_selector(|| "integration-access-card".into())
        .child(
            row("Диктовка — Groq", "API-ключ для облачной расшифровки речи")
                .child(badge(key_status, MUTED_FG())),
        );
    for (key, label) in [
        ("provider", "Провайдер"),
        ("language", "Язык"),
        ("hotkey", "Горячая клавиша"),
    ] {
        config = config.child(crate::async_fields::field_row(
            app,
            "secrets.config",
            label,
            |v| vstr(vget(v, "config"), key),
        ));
    }
    col = col.child(config);

    let config_slot = app.data("secrets.config");
    let cfg = vget(&config_slot, "config");
    let has_key = vbool(&config_slot, "hasApiKey") || vbool(cfg, "hasApiKey");
    if has_key {
        col = col.child(
            card().child(
                row("Использовать Groq для диктовки", "").child(
                    toggle(
                        "secrets-provider",
                        vbool(cfg, "providerEnabled"),
                        cx,
                        |this, checked, _| {
                            this.action(
                                "dictation.update_config",
                                json!({"providerEnabled": checked}),
                            );
                        },
                    )
                    .accessibility_label("Использовать Groq для диктовки"),
                ),
            ),
        );
    }

    let key_in = app.input("secrets.key", "gsk_…", true, window, cx);
    col = col.child(
        card()
            .child(
                div()
                    .text_size(px(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("API-ключ Groq"),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(input_field(&key_in).aria_label("API-ключ Groq")),
                    )
                    .child(btn(
                        "secrets-verify",
                        "Проверить",
                        false,
                        cx,
                        |this, cx| {
                            let key = this.input_value("secrets.key", cx);
                            if !key.is_empty() {
                                this.call(
                                    "secrets.verify",
                                    "dictation.verify_api_key",
                                    json!({"key": key}),
                                );
                            }
                        },
                    ))
                    .child(btn(
                        "secrets-save",
                        "Сохранить",
                        true,
                        cx,
                        |this, cx| {
                            let verified = vbool(&this.data("secrets.verify"), "valid");
                            let key = this.input_value("secrets.key", cx);
                            if verified && !key.is_empty() {
                                this.action("dictation.set_api_key", json!({"key": key}));
                            } else {
                                this.error = Some(
                                    concat!(
                                        concat!(
                                            "Сначала проверьте ключ — сохранение разрешено только ",
                                            "после успешной ",
                                        ),
                                        "проверки.",
                                    )
                                    .into(),
                                );
                            }
                        },
                    ))
                    .child(btn(
                        "secrets-clear",
                        "Удалить",
                        false,
                        cx,
                        |this, cx| {
                            this.ask_confirm(
                                "Удалить ключ",
                                "Диктовка через Groq перестанет работать.",
                                "dictation.clear_api_key",
                                json!({}),
                                cx,
                            );
                        },
                    )),
            )
            .child(div().flex().gap_2().child(btn(
                "secrets-test",
                concat!("Тест соедине", "ния",),
                false,
                cx,
                |this, _| {
                    this.call("secrets.test", "dictation.test_connectivity", json!({}));
                },
            ))),
    );

    let verify = app.data("secrets.verify");
    if !verify.is_null() {
        let ok = vbool(&verify, "valid");
        let msg = vopt(&verify, "message").unwrap_or_else(|| {
            if ok {
                "Ключ принят.".into()
            } else {
                "Ключ отклонён.".into()
            }
        });
        col = col.child(card().child(row("Проверка ключа", msg).child(badge(
            if ok { "OK" } else { "Отклонён" },
            if ok { SUCCESS() } else { DESTRUCTIVE() },
        ))));
    }
    let test = app.data("secrets.test");
    if !test.is_null() {
        let ok = vbool(&test, "ok") || vbool(&test, "reachable");
        col = col.child(
            card().child(
                row(
                    "Тест соединения",
                    vopt(&test, "message")
                        .unwrap_or_else(|| serde_json::to_string(&test).unwrap_or_default()),
                )
                .child(badge(
                    if ok {
                        "Доступен"
                    } else {
                        "Недоступен"
                    },
                    if ok { SUCCESS() } else { DESTRUCTIVE() },
                )),
            ),
        );
    }

    col = col.child(slot_or(app, "secrets.stats", |v| {
        card()
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(c(MUTED_FG()))
                    .child("Статистика диктовки"),
            )
            .child(kv("Сессий", vstr(v, "totalSessions")))
            .child(kv("Слов", vstr(v, "totalWords")))
            .child(kv(
                "Записано",
                format!("{:.0} сек.", vnum(v, "totalRecordSeconds")),
            ))
            .into_any_element()
    }));

    col.into_any_element()
}
