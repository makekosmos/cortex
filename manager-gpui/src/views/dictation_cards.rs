//! Диктовка — secondary cards split out of dictation.rs (source-size gate):
//! параметры `dictation.update_config`, статистика, pending-очередь и
//! локальные модели.
use ::gpui::{prelude::*, *};
use imago_gpui::button;
use serde_json::json;

use crate::app::ManagerApp;
use crate::widgets::*;
use mundus_gpui_kit::theme::*;

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
    el = el.child(
        row("Диктовка включена", "providerEnabled").child(
            toggle(
                "dictation-enabled",
                vbool(cfg, "providerEnabled"),
                cx,
                |this, checked, _| {
                    this.action(
                        "dictation.update_config",
                        json!({"providerEnabled": checked}),
                    );
                },
            )
            .accessibility_label("Диктовка включена"),
        ),
    );
    el = el.child(
        row(
            "Вставлять текст автоматически",
            "injectMode: auto_paste → Ctrl+V в целевое окно; иначе только clipboard",
        )
        .child(
            toggle(
                "dictation-autopaste",
                vstr(cfg, "injectMode") != "clipboard_only",
                cx,
                |this, checked, _| {
                    this.action(
                        "dictation.update_config",
                        json!({"injectMode": if checked { "auto_paste" } else { "clipboard_only" }}),
                    );
                },
            )
            .accessibility_label("Вставлять текст автоматически"),
        ),
    );
    el = el.child(
        row(
            "Приглушать звук при записи",
            "duckAudioDuringRecording — понижает системную громкость",
        )
        .child(
            toggle(
                "dictation-duck",
                vbool(cfg, "duckAudioDuringRecording"),
                cx,
                |this, checked, _| {
                    this.action(
                        "dictation.update_config",
                        json!({"duckAudioDuringRecording": checked}),
                    );
                },
            )
            .accessibility_label("Приглушать звук при записи"),
        ),
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
                .accessibility_label(format!("Язык распознавания: {lang}"))
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
