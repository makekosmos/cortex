//! Диктовка — secondary cards split out of dictation.rs (source-size gate):
//! параметры `dictation.update_config`, статистика, pending-очередь и
//! локальные модели.
use ::gpui::{prelude::*, *};
use imago_gpui::button;
use serde_json::json;

use crate::app::ManagerApp;
use crate::widgets::*;
use kosmos_gpui_kit::theme::*;

// --- Конфиг ------------------------------------------------------------------

pub(crate) fn config_card(app: &mut ManagerApp, cx: &mut Context<ManagerApp>) -> AnyElement {
    let config_slot = app.data("dictation.state");
    let cfg = vget(&config_slot, "config");
    if cfg.is_null() {
        return div().into_any_element();
    }
    let mut el = card().child(
        div()
            .text_size(px(12.))
            .text_color(c(MUTED_FG()))
            .child("Параметры (dictation.update_config)"),
    );
    el = el.child(row("Диктовка включена", "providerEnabled").child(toggle(
        "dictation-enabled",
        vbool(cfg, "providerEnabled"),
        cx,
        |this, checked, _| {
            this.action(
                "dictation.update_config",
                json!({"providerEnabled": checked}),
            );
        },
    )));
    el = el.child(
        row(
            "Вставлять текст автоматически",
            "injectMode: auto_paste → Ctrl+V в целевое окно; иначе только clipboard",
        )
        .child(toggle(
            "dictation-autopaste",
            vstr(cfg, "injectMode") != "clipboard_only",
            cx,
            |this, checked, _| {
                this.action(
                    "dictation.update_config",
                    json!({"injectMode": if checked { "auto_paste" } else { "clipboard_only" }}),
                );
            },
        )),
    );
    el = el.child(
        row(
            "Приглушать звук при записи",
            "duckAudioDuringRecording — понижает системную громкость",
        )
        .child(toggle(
            "dictation-duck",
            vbool(cfg, "duckAudioDuringRecording"),
            cx,
            |this, checked, _| {
                this.action(
                    "dictation.update_config",
                    json!({"duckAudioDuringRecording": checked}),
                );
            },
        )),
    );
    el = el.child(
        row("Язык распознавания", "Whisper language hint").child(div().flex().gap_2().children(
            ["ru", "en", "auto"].map(|lang| {
                let active = vstr(cfg, "language") == lang;
                button::button(
                    SharedString::from(format!("dictation-lang-{lang}")),
                    if active {
                        button::ButtonKind::Secondary
                    } else {
                        button::ButtonKind::Ghost
                    },
                )
                .label(lang)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.action("dictation.update_config", json!({"language": lang}));
                    cx.notify();
                }))
            }),
        )),
    );
    el.into_any_element()
}

// --- Статистика ----------------------------------------------------------------

pub(crate) fn stats_card(app: &mut ManagerApp, cx: &mut Context<ManagerApp>) -> AnyElement {
    slot_or(app, "dictation.stats", |v| {
        let mut el = card().child(row("Статистика", "Накоплено по сессиям диктовки").child(
            btn_id("dictation-stats-reset", "Сбросить", {
                cx.listener(|this, _, _, cx| {
                    this.ask_confirm(
                        "Сбросить статистику диктовки",
                        "Счётчики слов и сессий обнулятся.",
                        "dictation.reset_stats",
                        json!({}),
                        cx,
                    );
                })
            }),
        ));
        el = el.child(kv("Сессий", vstr(v, "totalSessions")));
        el = el.child(kv("Слов", vstr(v, "totalWords")));
        el = el.child(kv(
            "Записано",
            format!("{:.0} сек.", vnum(v, "totalRecordSeconds")),
        ));
        el = el.child(kv("Скорость", format!("{:.0} слов/мин", vnum(v, "wpm"))));
        el = el.child(kv(
            "Сэкономлено",
            format!("{:.0} сек. печати", vnum(v, "timeSavedSeconds")),
        ));
        el.into_any_element()
    })
}

// --- Очередь pending -----------------------------------------------------------

pub(crate) fn pending_card(app: &mut ManagerApp, cx: &mut Context<ManagerApp>) -> AnyElement {
    slot_or(app, "dictation.pending", |v| {
        let items = varr(v, "items");
        let mut el = card().child(
            row(
                "Очередь на распознавание",
                "Аудио на диске — повтор после сбоя сети/ключа",
            )
            .child(badge(format!("{}", items.len()), MUTED_FG())),
        );
        if items.is_empty() {
            el = el.child(empty("Очередь пуста"));
        }
        for item in items.iter().take(20) {
            let uuid = vstr(item, "uuid");
            let retry_uuid = uuid.clone();
            let discard_uuid = uuid.clone();
            el = el.child(
                row(
                    uuid.chars().take(8).collect::<String>(),
                    format!(
                        "{} · {:.0} сек. · попыток: {:.0}{}",
                        vstr(item, "createdAt"),
                        vnum(item, "durationSec"),
                        vnum(item, "attempts"),
                        vopt(item, "lastError")
                            .map(|e| format!(" · {e}"))
                            .unwrap_or_default()
                    ),
                )
                .child(btn_id(
                    &format!("dict-retry-{uuid}"),
                    "Повторить",
                    {
                        cx.listener(move |this, _, _, cx| {
                            this.action("dictation.retry", json!({"uuid": retry_uuid}));
                            cx.notify();
                        })
                    },
                ))
                .child(btn_id(&format!("dict-discard-{uuid}"), "Удалить", {
                    cx.listener(move |this, _, _, cx| {
                        this.action("dictation.discard", json!({"uuid": discard_uuid}));
                        cx.notify();
                    })
                })),
            );
        }
        if !items.is_empty() {
            el = el.child(div().flex().gap_2().child(btn_id(
                "dict-retry-all",
                "Повторить все",
                {
                    cx.listener(|this, _, _, cx| {
                        this.action("dictation.retry_all", json!({}));
                        cx.notify();
                    })
                },
            )));
        }
        el.into_any_element()
    })
}

// --- Локальная модель ------------------------------------------------------------

pub(crate) fn local_card(app: &mut ManagerApp, cx: &mut Context<ManagerApp>) -> AnyElement {
    let local = app.data("dictation.local");
    let models = app.data("dictation.models");
    if local.is_null() && models.is_null() {
        return div().into_any_element();
    }
    let mut el = card().child(
        div()
            .text_size(px(12.))
            .text_color(c(MUTED_FG()))
            .child("Локальная модель (on-device STT)"),
    );
    if !local.is_null() {
        el = el.child(kv(
            "Движок",
            format!(
                "{} · {}",
                if vbool(&local, "warm") {
                    "прогрет"
                } else {
                    "холодный"
                },
                vopt(&local, "backend").unwrap_or_else(|| "—".into())
            ),
        ));
        if let Some(model) = vopt(&local, "loadedModel") {
            el = el.child(kv("Загружена", model));
        }
        if let Some(err) = vopt(&local, "error") {
            el = el.child(kv("Ошибка движка", err));
        }
    }
    for model in varr(&models, "models").iter().take(10) {
        let id = vstr(model, "id");
        let use_id = id.clone();
        let download_id = id.clone();
        let delete_id = id.clone();
        let mut r = row(
            vstr(model, "name"),
            format!(
                "{} · {:.0} МБ{}",
                vstr(model, "description"),
                vnum(model, "sizeMb"),
                if vbool(model, "recommended") {
                    " · рекомендуется"
                } else {
                    ""
                }
            ),
        );
        if vbool(model, "selected") {
            r = r.child(badge("Выбрана", SUCCESS()));
        } else if vbool(model, "downloaded") {
            r = r
                .child(badge("Скачана", MUTED_FG()))
                .child(btn_id(
                    &format!("dict-use-{id}"),
                    "Использовать",
                    {
                        cx.listener(move |this, _, _, cx| {
                            this.action("dictation.use_local_model", json!({"modelId": use_id}));
                            cx.notify();
                        })
                    },
                ))
                .child(btn_id(&format!("dict-del-{id}"), "Удалить", {
                    cx.listener(move |this, _, _, cx| {
                        this.ask_confirm(
                            "Удалить локальную модель",
                            "Файлы модели будут удалены с диска.",
                            "dictation.delete_local_model",
                            json!({"modelId": delete_id}),
                            cx,
                        );
                    })
                }));
        } else {
            r = r.child(btn_id(&format!("dict-dl-{id}"), "Скачать", {
                cx.listener(move |this, _, _, cx| {
                    this.action(
                        "dictation.download_local_model",
                        json!({"modelId": download_id, "select": true}),
                    );
                    cx.notify();
                })
            }));
        }
        el = el.child(r);
    }
    el =
        el.child(div().text_size(px(12.)).text_color(c(MUTED_FG())).child(
            "Прогресс скачивания приходит по WS-событиям — обновите виджет после загрузки.",
        ));
    el.into_any_element()
}
