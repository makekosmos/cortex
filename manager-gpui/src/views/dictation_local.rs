//! Диктовка — локальная on-device модель: список моделей, скачивание,
//! выбор и удаление (split from dictation_cards.rs for the source-size gate).
use ::gpui::{prelude::*, *};
use serde_json::json;

use crate::app::ManagerApp;
use crate::widgets::*;
use kosmos_gpui_kit::theme::*;

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
    // Live download progress from the WS event slot
    // (`dictation_local_model_download_progress`, app.rs::handle_engine_event).
    let download = app.data("dictation.download");
    if !download.is_null() {
        let model = vopt(&download, "modelId").unwrap_or_else(|| "модель".into());
        let percent = vnum(&download, "percent");
        let text = if percent > 0.0 {
            format!("{model} — {percent:.0}%")
        } else {
            format!("{model}…")
        };
        el = el.child(kv("Скачивание", text));
    }
    el.into_any_element()
}
