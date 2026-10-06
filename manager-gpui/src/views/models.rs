//! Модели — локальные on-device STT модели. Каталог и файлы принадлежат
//! Engine; dictation-gpui только потребляет выбранную модель, а управление
//! (скачать / выбрать / удалить) живёт здесь (dictation `models_card`
//! parity: `dictation.list_local_models`,
//! `dictation.{download,use,delete}_local_model`). Прогресс скачивания
//! приходит внутри snapshot'а (`downloading`/`downloadPercent`) — Manager
//! не подписан на WS, поэтому `app_polls` перечитывает список раз в секунду,
//! пока хоть одна модель качается.
use ::gpui::{prelude::*, *};
use serde_json::{json, Value};

use crate::app::ManagerApp;
use crate::theme::*;
use crate::widgets::*;

/// Action column width — identical for the size button and the progress
/// fill so rows never jump when a download starts.
const ACTION_W: f32 = 96.;
const ACTION_H: f32 = 28.;
const PIP_W: f32 = 14.;
const PIP_H: f32 = 4.;
/// Per-frame lerp toward the polled percent — engine updates land once a
/// second, the fill still moves smoothly between them.
const FILL_LERP: f32 = 0.3;

pub fn load(app: &mut ManagerApp) {
    app.call("models.list", "dictation.list_local_models", json!({}));
}

fn icon_for(id: &str) -> &'static str {
    if id.starts_with("parakeet") {
        "icons/providers/nvidia.svg"
    } else {
        "icons/providers/openai.svg"
    }
}

/// Five thin bars; `score` is 0.0..=1.0 from the Engine catalog.
fn pips(score: f64) -> Div {
    let filled = (score * 5.0).round().clamp(0.0, 5.0) as usize;
    let mut row = div().flex_none().flex().items_center().gap(px(2.));
    for index in 0..5 {
        row = row.child(
            div()
                .w(px(PIP_W))
                .h(px(PIP_H))
                .rounded(px(1.5))
                .bg(if index < filled {
                    c(ACCENT())
                } else {
                    fade(FG(), 0.12)
                }),
        );
    }
    row
}

/// Fixed-size ghost button: size label until the download starts.
fn download_button(id: &str, label: String, cx: &mut Context<ManagerApp>) -> Stateful<Div> {
    let model_id = id.to_owned();
    div()
        .id(SharedString::from(format!("model-dl-{id}")))
        .debug_selector(move || format!("model-dl-{id}"))
        .w(px(ACTION_W))
        .h(px(ACTION_H))
        .flex_none()
        .rounded_full()
        .bg(fade(FG(), 0.08))
        .cursor_pointer()
        .role(Role::Button)
        .aria_label(format!("Скачать {id}"))
        .hover(|button| button.bg(fade(FG(), 0.14)))
        .flex()
        .items_center()
        .justify_center()
        .text_size(ui_px(12.5))
        .font_weight(FontWeight::MEDIUM)
        .text_color(c(FG()))
        .child(label)
        .on_click(cx.listener(move |this, _, _, _| {
            this.action(
                "dictation.download_local_model",
                json!({"modelId": model_id, "select": true}),
            );
        }))
}

/// Same fixed-size pill as an accent progress fill — the right edge of the
/// fill lightens via a horizontal gradient so the front reads as moving.
fn progress_pill(id: &str, percent: Option<f32>) -> Stateful<Div> {
    let fraction = percent.unwrap_or(0.0).clamp(0.0, 100.0) / 100.0;
    let label = percent
        .map(|percent| format!("{percent:.0}%"))
        .unwrap_or_else(|| "…".into());
    div()
        .id(SharedString::from(format!("model-dl-{id}")))
        .debug_selector(move || format!("model-dl-{id}"))
        .w(px(ACTION_W))
        .h(px(ACTION_H))
        .flex_none()
        .rounded_full()
        .bg(fade(FG(), 0.08))
        .relative()
        .overflow_hidden()
        .role(Role::ProgressIndicator)
        .aria_label(format!("Скачивание {id}"))
        .child(
            div()
                .absolute()
                .left_0()
                .top_0()
                .bottom_0()
                .w(px(ACTION_W * fraction))
                .bg(linear_gradient(
                    90.,
                    linear_color_stop(c(ACCENT()), 0.),
                    linear_color_stop(mix(0xffffff, 0.28, ACCENT()), 1.),
                )),
        )
        .child(
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .text_size(ui_px(12.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(c(FG()))
                .child(label),
        )
}

pub fn render(
    app: &mut ManagerApp,
    window: &mut Window,
    cx: &mut Context<ManagerApp>,
) -> AnyElement {
    let models = app.data("models.list");
    let mut sorted: Vec<Value> = varr(&models, "models").to_vec();
    sorted.sort_by(|a, b| {
        vnum(b, "speedScore")
            .total_cmp(&vnum(a, "speedScore"))
            .then(vnum(b, "accuracyScore").total_cmp(&vnum(a, "accuracyScore")))
    });

    let mut rows = card()
        .id("models-card")
        .debug_selector(|| "models-card".into())
        .px(px(0.))
        .py(px(0.))
        .gap(px(0.));
    let mut downloading_ids = std::collections::HashSet::new();
    let mut animating = false;
    for (index, model) in sorted.iter().enumerate() {
        let id = vstr(model, "id");
        let downloading = vbool(model, "downloading");
        // Outer Option = render a progress pill; inner None = the phase has
        // no measurable percent yet (engine side reported None).
        let pill: Option<Option<f32>> = if downloading {
            downloading_ids.insert(id.clone());
            match model.get("downloadPercent").and_then(Value::as_f64) {
                Some(target) => {
                    let target = target as f32;
                    let current = app.model_fill.get(&id).copied().unwrap_or(0.0);
                    let mut next = current + (target - current) * FILL_LERP;
                    if (target - next).abs() < 0.4 {
                        next = target;
                    }
                    app.model_fill.insert(id.clone(), next);
                    animating |= next < target;
                    Some(Some(next))
                }
                None => Some(None),
            }
        } else {
            None
        };

        let use_id = id.clone();
        let delete_id = id.clone();
        let sel_id = id.clone();
        let mut r = div()
            .id(SharedString::from(format!("model-{id}")))
            .debug_selector(move || format!("model-{sel_id}"))
            .mx(px(16.))
            .py(px(12.))
            .min_h(px(52.))
            .when(index > 0, |row| {
                row.border_t_1().border_color(fade(BORDER(), 0.6))
            })
            .flex()
            .items_center()
            .gap(px(12.))
            .child(
                svg()
                    .path(icon_for(&id))
                    .size(px(20.))
                    .flex_none()
                    .text_color(c(FG())),
            )
            .child(row_copy(vstr(model, "name"), ""))
            .child(pips(vnum(model, "speedScore")))
            .child(pips(vnum(model, "accuracyScore")));
        if let Some(percent) = pill {
            r = r.child(progress_pill(&id, percent));
        } else if vbool(model, "selected") {
            r = r.child(badge("Выбрана", SUCCESS())).child(btn_id(
                &format!("model-del-{id}"),
                "Удалить",
                {
                    cx.listener(move |this, _, _, cx| {
                        this.ask_confirm(
                            format!("Удалить {delete_id}?"),
                            "Файлы модели будут удалены с диска",
                            "dictation.delete_local_model",
                            json!({"modelId": delete_id}),
                            cx,
                        );
                    })
                },
            ));
        } else if vbool(model, "downloaded") {
            r = r.child(btn_id(
                &format!("model-use-{id}"),
                "Использовать",
                cx.listener(move |this, _, _, _| {
                    this.action("dictation.use_local_model", json!({"modelId": use_id}));
                }),
            ));
        } else {
            r = r.child(download_button(
                &id,
                format!("{:.0} МБ", vnum(model, "sizeMb")),
                cx,
            ));
        }
        rows = rows.child(r);
    }
    // Drop fill state for models no longer downloading; keep animating while
    // the lerped fill is still catching up to the polled percent.
    app.model_fill.retain(|id, _| downloading_ids.contains(id));
    if animating {
        window.request_animation_frame();
    }
    if models.is_null() {
        rows = rows.child(empty("Загрузка…"));
    }
    rows.into_any_element()
}
