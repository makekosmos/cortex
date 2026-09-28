//! Диктовка — DictationTab.vue parity: состояние state machine, конфиг
//! (hotkey/язык/режимы — display + переключатели через `dictation.update_config`),
//! очередь pending (`dictation.list_pending`/`retry`/`discard`), статистика
//! (`dictation.get_stats`/`reset_stats`), локальные модели
//! (`dictation.local_status`/`list_local_models`/`use_local_model`/
//! `download_local_model`/`delete_local_model`) и запуск pill-оверлея записи
//! (Engine-owned WASAPI capture — `dictation.capture.*` + `speech.transcribe`).
//!
//! WS-канал событий Engine (`worker.rs` → `ManagerApp::handle_engine_event`):
//! глобальная горячая клавиша запускает pill (`dictation_toggle_trigger` /
//! `dictation_ptt_trigger`), live-прогресс скачивания модели живёт в слоте
//! `dictation.download`. Не перенесено из Vue: захват нового хоткея —
//! `begin_hotkey_capture` здесь не вызывается; capture-события складываются
//! в слот `dictation.capture` для будущей строки назначения клавиши.
use ::gpui::{prelude::*, *};
use gpui_component::Disableable;
use imago_gpui::button;
use serde_json::json;

use crate::app::ManagerApp;
use crate::views::{dictation_cards, dictation_local};
use crate::widgets::*;
use mundus_gpui_kit::theme::*;

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

    col = col.child(state_card(app, cx));
    col = col.child(record_card(app, cx));
    col = col.child(result_card(app));
    // Secondary cards live in dictation_cards.rs (source-size gate).
    col = col.child(dictation_cards::config_card(app, cx));
    col = col.child(dictation_cards::stats_card(app, cx));
    col = col.child(dictation_cards::pending_card(app, cx));
    col = col.child(dictation_local::local_card(app, cx));

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

fn state_card(app: &mut ManagerApp, cx: &mut Context<ManagerApp>) -> AnyElement {
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
        let capturing = app.hotkey_capturing;
        let mut hotkey_row = row(
            "Горячая клавиша",
            if capturing {
                "Нажмите комбинацию… (Esc — отмена)".to_string()
            } else {
                vstr(cfg, "hotkey")
            },
        );
        if capturing {
            hotkey_row = hotkey_row.child(badge("Захват", WARN()));
        } else {
            hotkey_row = hotkey_row.child(
                button::ghost("dict-hotkey-change")
                    .label("Изменить")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.hotkey_capturing = true;
                        this.action("dictation.begin_hotkey_capture", json!({}));
                        cx.notify();
                    })),
            );
        }
        el = el.child(hotkey_row);
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
    // The pill overlay and session orchestration live in the standalone
    // dictation-gpui app — this card only mirrors Engine state and issues
    // Engine-level start/cancel.
    let state = vstr(&app.data("dictation.state"), "state");
    let (label, hint): (&str, &str) = match state.as_str() {
        "recording" => (
            "Идёт запись",
            "Pill-оверлей показывает приложение Dictation; стоп — по хоткею",
        ),
        "transcribing" | "waiting" => ("Распознаю…", "speech.transcribe в Engine"),
        _ => (
            "Начать запись",
            "WASAPI-захват на стороне Engine, результат вставляется/копируется по injectMode",
        ),
    };
    let recording = state == "recording";
    let busy = recording || state == "transcribing" || state == "waiting";
    card()
        .child(
            row("Запись", hint)
                .child(
                    button::primary("dictation-toggle")
                        .label(label)
                        .disabled(busy)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.action("dictation.capture.start", json!({}));
                            cx.notify();
                        })),
                )
                .when(recording, |el| {
                    el.child(button::ghost("dictation-cancel").label("Отмена").on_click(
                        cx.listener(|this, _, _, cx| {
                            this.action("dictation.cancel", json!({}));
                            cx.notify();
                        }),
                    ))
                }),
        )
        .child(div().text_size(px(12.)).text_color(c(MUTED_FG())).child(
            "Pill-оверлей и глобальная горячая клавиша — отдельное приложение \
             Dictation (dictation-gpui) поверх событий Engine.",
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
