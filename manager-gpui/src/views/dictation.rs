//! Диктовка — DictationTab.vue parity: состояние state machine, конфиг
//! (hotkey/язык/режимы — display + переключатели через `dictation.update_config`),
//! очередь pending (`dictation.list_pending`/`retry`/`discard`), статистика
//! (`dictation.get_stats`/`reset_stats`), локальные модели
//! (`dictation.local_status`/`list_local_models`/`use_local_model`/
//! `download_local_model`/`delete_local_model`) и запуск pill-оверлея записи
//! (Engine-owned WASAPI capture — `dictation.capture.*` + `speech.transcribe`).
//!
//! Не перенесено из Vue (нужен WS-канал событий Engine — HTTP клиент его не
//! подписывает): захват нового хоткея (`begin_hotkey_capture` отдаёт клавиши
//! через broadcast `dictation_capture_key`), live-прогресс скачивания модели и
//! запуск записи по глобальной горячей клавише.
use ::gpui::{prelude::*, *};
use gpui_component::Disableable;
use imago_gpui::button;
use serde_json::json;

use crate::app::ManagerApp;
use crate::pill::PillPhase;
use crate::views::dictation_cards;
use crate::widgets::*;
use kosmos_gpui_kit::theme::*;

pub fn load(app: &mut ManagerApp) {
    app.call("dictation.state", "dictation.get_state", json!({}));
    app.call("dictation.stats", "dictation.get_stats", json!({}));
    app.call("dictation.pending", "dictation.list_pending", json!({}));
    app.call("dictation.local", "dictation.local_status", json!({}));
    app.call("dictation.models", "dictation.list_local_models", json!({}));
}

pub fn render(
    app: &mut ManagerApp,
    _window: &mut Window,
    cx: &mut Context<ManagerApp>,
) -> AnyElement {
    let mut col = div().flex().flex_col().gap_4().w_full();
    col = col.child(section(
        "Диктовка",
        "Голосовой ввод: состояние, модель, очередь распознавания",
    ));

    col = col.child(state_card(app));
    col = col.child(record_card(app, cx));
    col = col.child(result_card(app));
    // Secondary cards live in dictation_cards.rs (source-size gate).
    col = col.child(dictation_cards::config_card(app, cx));
    col = col.child(dictation_cards::stats_card(app, cx));
    col = col.child(dictation_cards::pending_card(app, cx));
    col = col.child(dictation_cards::local_card(app, cx));

    col.into_any_element()
}

// --- Состояние --------------------------------------------------------------

fn state_label(state: &str) -> &'static str {
    match state {
        "recording" => "Запись",
        "transcribing" => "Распознаю",
        "pending" => "Ждёт сети",
        "error" => "Ошибка",
        _ => "Ожидание",
    }
}

fn state_color(state: &str) -> u32 {
    match state {
        "recording" => DESTRUCTIVE(),
        "transcribing" | "pending" => WARN(),
        "error" => DESTRUCTIVE(),
        _ => SUCCESS(),
    }
}

fn state_card(app: &mut ManagerApp) -> AnyElement {
    slot_or(app, "dictation.state", |v| {
        let state = vstr(v, "state");
        let cfg = vget(v, "config");
        let has_key = vbool(v, "hasApiKey");
        let mut el = card();
        el = el.child(
            row(
                "Состояние",
                vopt(v, "lastError").unwrap_or_else(|| {
                    if vnum(v, "attempts") > 0.0 {
                        format!("Попыток: {:.0}", vnum(v, "attempts"))
                    } else {
                        "Бэкенд: runtime/src/dictation (state machine в Engine)".into()
                    }
                }),
            )
            .child(badge(state_label(&state), state_color(&state))),
        );
        el = el.child(kv("Горячая клавиша", vstr(cfg, "hotkey")));
        el = el.child(kv(
            "Режим триггера",
            trigger_label(&vstr(cfg, "triggerMode")),
        ));
        el = el.child(kv("Язык", vstr(cfg, "language")));
        el = el.child(kv(
            "Провайдер",
            format!(
                "{}{}",
                vstr(cfg, "provider"),
                if vbool(cfg, "providerEnabled") {
                    ""
                } else {
                    " (выключен)"
                }
            ),
        ));
        el = el.child(kv("Модель", vstr(cfg, "model")));
        el = el.child(
            row("API-ключ Groq", "Управляется в разделе «Ключи»").child(badge(
                if has_key {
                    "Задан"
                } else {
                    "Не задан"
                },
                if has_key { SUCCESS() } else { WARN() },
            )),
        );
        el.into_any_element()
    })
}

fn trigger_label(mode: &str) -> &'static str {
    match mode {
        "push_to_talk" => "Удержание (push-to-talk)",
        _ => "Переключение (toggle)",
    }
}

// --- Запись (pill) ----------------------------------------------------------

fn record_card(app: &mut ManagerApp, cx: &mut Context<ManagerApp>) -> AnyElement {
    let phase = app.pill_phase;
    let (label, hint): (&str, &str) = match phase {
        Some(PillPhase::Starting) => ("Запуск записи…", "Engine открывает захват микрофона"),
        Some(PillPhase::Recording) => (
            "Остановить запись",
            "Идёт запись — pill-окно у нижнего края экрана",
        ),
        Some(PillPhase::Processing) => ("Распознаю…", "capture.stop → speech.transcribe"),
        None => (
            "Начать запись",
            "WASAPI-захват на стороне Engine, результат вставляется/копируется по injectMode",
        ),
    };
    let busy = phase.is_some();
    card()
        .child(
            row("Pill-оверлей", hint)
                .child(
                    button::primary("dictation-toggle")
                        .label(label)
                        .when(busy && phase != Some(PillPhase::Recording), |b| {
                            b.disabled(true)
                        })
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.dictation_toggle(cx);
                        })),
                )
                .when(phase.is_some(), |el| {
                    el.child(button::ghost("dictation-cancel").label("Отмена").on_click(
                        cx.listener(|this, _, _, cx| {
                            this.dictation_cancel(cx);
                        }),
                    ))
                }),
        )
        .child(div().text_size(px(12.)).text_color(c(MUTED_FG())).child(
            "Глобальная горячая клавиша пока приходит только в Electron Host \
                     (события Engine по WS; у GPUI-клиента только HTTP).",
        ))
        .into_any_element()
}

// --- Последняя расшифровка ---------------------------------------------------

fn result_card(app: &mut ManagerApp) -> AnyElement {
    let result = app.data("dictation.result");
    if result.is_null() {
        return div().into_any_element();
    }
    let text = vopt(&result, "text").unwrap_or_default();
    let delivery = vstr(&result, "delivery");
    let sub = match vopt(&result, "error") {
        Some(e) => e,
        None => format!(
            "Доставка: {} · {:.0} мс",
            if delivery.is_empty() {
                "—".into()
            } else {
                delivery
            },
            vnum(&result, "durationMs")
        ),
    };
    card()
        .child(
            row(
                if text.is_empty() {
                    "Последняя расшифровка".to_string()
                } else {
                    text
                },
                sub,
            )
            .child(badge(
                if vstr(&result, "state") == "error" {
                    "Ошибка"
                } else {
                    "Готово"
                },
                if vstr(&result, "state") == "error" {
                    DESTRUCTIVE()
                } else {
                    SUCCESS()
                },
            )),
        )
        .into_any_element()
}
