//! Модели — локальные on-device STT модели. Каталог и файлы принадлежат
//! Engine; dictation-gpui только потребляет выбранную модель, а управление
//! (скачать / выбрать / удалить) живёт здесь (dictation `models_card`
//! parity: `dictation.local_status`, `dictation.list_local_models`,
//! `dictation.{download,use,delete}_local_model`).
use ::gpui::{prelude::*, *};
use serde_json::json;

use crate::app::ManagerApp;
use crate::theme::*;
use crate::widgets::*;

pub fn load(app: &mut ManagerApp) {
    app.call("models.local", "dictation.local_status", json!({}));
    app.call("models.list", "dictation.list_local_models", json!({}));
}

pub fn render(
    app: &mut ManagerApp,
    _window: &mut Window,
    cx: &mut Context<ManagerApp>,
) -> AnyElement {
    let local = app.data("models.local");
    let models = app.data("models.list");
    let mut el = card()
        .id("models-card")
        .debug_selector(|| "models-card".into())
        .child(
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
    }
    for model in varr(&models, "models").iter().take(10) {
        let id = vstr(model, "id");
        let use_id = id.clone();
        let download_id = id.clone();
        let delete_id = id.clone();
        let mut r = row(
            vstr(model, "name"),
            format!(
                "{:.0} МБ{}",
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
                    &format!("model-use-{id}"),
                    "Использовать",
                    {
                        cx.listener(move |this, _, _, _| {
                            this.action("dictation.use_local_model", json!({"modelId": use_id}));
                        })
                    },
                ))
                .child(btn_id(&format!("model-del-{id}"), "Удалить", {
                    cx.listener(move |this, _, _, cx| {
                        this.ask_confirm(
                            format!("Удалить {delete_id}?"),
                            "Файлы модели будут удалены с диска",
                            "dictation.delete_local_model",
                            json!({"modelId": delete_id}),
                            cx,
                        );
                    })
                }));
        } else {
            r = r.child(btn_id(&format!("model-dl-{id}"), "Скачать", {
                cx.listener(move |this, _, _, _| {
                    this.action(
                        "dictation.download_local_model",
                        json!({"modelId": download_id, "select": true}),
                    );
                })
            }));
        }
        el = el.child(r);
    }
    if local.is_null() && models.is_null() {
        el = el.child(empty("Загрузка…"));
    }
    el.into_any_element()
}
