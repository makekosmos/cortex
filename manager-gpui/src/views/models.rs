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
/// Fixed column for one pip group — header labels sit on the same grid.
const PIP_COL_W: f32 = 84.;
/// Progress ring stroke width on the download pill.
const RING_W: f32 = 1.5;
/// Per-frame lerp toward the polled percent — engine updates land once a
/// second, the fill still moves smoothly between them.
const FILL_LERP: f32 = 0.3;

pub fn load(app: &mut ManagerApp) {
    app.call("models.list", "dictation.list_local_models", json!({}));
}

fn icon_for(id: &str) -> AnyElement {
    if id == "parakeet-ultra" {
        img("icons/providers/moondream.webp")
            .size(px(20.))
            .flex_none()
            .into_any_element()
    } else {
        svg()
            .path(if id.starts_with("parakeet") {
                "icons/providers/nvidia.svg"
            } else {
                "icons/providers/openai.svg"
            })
            .size(px(20.))
            .flex_none()
            .text_color(c(FG()))
            .into_any_element()
    }
}

/// Column labels aligned to the row grid: icon spacer, name, the two pip
/// groups, the action pill.
fn header_row() -> Div {
    let cell = |label: &'static str| {
        div()
            .w(px(PIP_COL_W))
            .flex_none()
            .text_size(ui_px(11.))
            .text_color(c(MUTED_FG()))
            .child(label)
    };
    div()
        .mx(px(16.))
        .pt(px(12.))
        .pb(px(6.))
        .border_b_1()
        .border_color(fade(BORDER(), 0.6))
        .flex()
        .items_center()
        .gap(px(12.))
        .child(div().w(px(20.)).flex_none())
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(ui_px(11.))
                .text_color(c(MUTED_FG()))
                .child("Модель"),
        )
        .child(cell("Скорость"))
        .child(cell("Точность"))
        .child(div().w(px(ACTION_W)).flex_none())
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

/// Fixed-size ghost pill — the single action shape on this page (size →
/// download, «Удалить» for downloaded). Same geometry/colors so rows never
/// change their action footprint.
fn pill_button(
    id: &str,
    label: String,
    aria: String,
    danger: bool,
    on_click: impl Fn(&mut ManagerApp, &mut Window, &mut Context<ManagerApp>) + 'static,
    cx: &mut Context<ManagerApp>,
) -> Stateful<Div> {
    div()
        .id(SharedString::from(id.to_string()))
        .debug_selector(move || id.to_string())
        .w(px(ACTION_W))
        .h(px(ACTION_H))
        .flex_none()
        .rounded_full()
        .bg(if danger {
            fade(DESTRUCTIVE(), 0.14)
        } else {
            fade(FG(), 0.08)
        })
        .cursor_pointer()
        .role(Role::Button)
        .aria_label(aria)
        .hover(|button| {
            button.bg(if danger {
                fade(DESTRUCTIVE(), 0.22)
            } else {
                fade(FG(), 0.14)
            })
        })
        .flex()
        .items_center()
        .justify_center()
        .text_size(ui_px(12.5))
        .font_weight(FontWeight::MEDIUM)
        .text_color(if danger { c(DESTRUCTIVE()) } else { c(FG()) })
        .child(label)
        .on_click(cx.listener(move |this, _, window, cx| on_click(this, window, cx)))
}

/// Size label until the download starts.
fn download_button(id: &str, label: String, cx: &mut Context<ManagerApp>) -> Stateful<Div> {
    let model_id = id.to_owned();
    pill_button(
        &format!("model-dl-{id}"),
        label,
        format!("Скачать {id}"),
        false,
        move |this, _, _| {
            this.action(
                "dictation.download_local_model",
                json!({"modelId": model_id, "select": true}),
            );
        },
        cx,
    )
}

/// Same pill for a downloaded model — selection lives in the dictation
/// app, so the only action here is delete.
fn delete_button(id: &str, cx: &mut Context<ManagerApp>) -> Stateful<Div> {
    let model_id = id.to_owned();
    pill_button(
        &format!("model-del-{id}"),
        "Удалить".into(),
        format!("Удалить {id}"),
        true,
        move |this, _, cx| {
            this.ask_confirm(
                format!("Удалить {model_id}?"),
                "Файлы модели будут удалены с диска",
                "dictation.delete_local_model",
                json!({"modelId": model_id}),
                cx,
            );
        },
        cx,
    )
}

/// Same fixed-size pill during download: a thin ring traced around the
/// pill's outline, filled clockwise from top-center to `percent`. The
/// lerped `percent` keeps the arc smooth between 1s polls.
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
        .role(Role::ProgressIndicator)
        .aria_label(format!("Скачивание {id}"))
        .child(
            canvas(
                |_, _, _| (),
                move |bounds, _, window, _| {
                    let inset = RING_W / 2.0;
                    let x0 = f32::from(bounds.origin.x) + inset;
                    let y0 = f32::from(bounds.origin.y) + inset;
                    let x1 = f32::from(bounds.origin.x + bounds.size.width) - inset;
                    let y1 = f32::from(bounds.origin.y + bounds.size.height) - inset;
                    let radius = ((y1 - y0) / 2.0).max(0.0);
                    let cx = (x0 + x1) / 2.0;
                    // Clockwise rounded-rect outline starting at top-center.
                    let trace = |path: &mut PathBuilder| {
                        path.move_to(point(px(cx), px(y0)));
                        path.line_to(point(px(x1 - radius), px(y0)));
                        path.arc_to(
                            point(px(radius), px(radius)),
                            px(0.),
                            false,
                            true,
                            point(px(x1), px(y0 + radius)),
                        );
                        path.line_to(point(px(x1), px(y1 - radius)));
                        path.arc_to(
                            point(px(radius), px(radius)),
                            px(0.),
                            false,
                            true,
                            point(px(x1 - radius), px(y1)),
                        );
                        path.line_to(point(px(x0 + radius), px(y1)));
                        path.arc_to(
                            point(px(radius), px(radius)),
                            px(0.),
                            false,
                            true,
                            point(px(x0), px(y1 - radius)),
                        );
                        path.line_to(point(px(x0), px(y0 + radius)));
                        path.arc_to(
                            point(px(radius), px(radius)),
                            px(0.),
                            false,
                            true,
                            point(px(x0 + radius), px(y0)),
                        );
                        path.line_to(point(px(cx), px(y0)));
                        path.close();
                    };
                    let mut track = PathBuilder::stroke(px(RING_W));
                    trace(&mut track);
                    window.paint_path(track.build().unwrap(), fade(ACCENT(), 0.22));
                    if fraction > 0.0 {
                        let perimeter = 2.0 * ((x1 - x0) - 2.0 * radius)
                            + 2.0 * ((y1 - y0) - 2.0 * radius)
                            + std::f32::consts::TAU * radius;
                        let mut ring = PathBuilder::stroke(px(RING_W))
                            .dash_array(&[px(fraction * perimeter), px(perimeter)]);
                        trace(&mut ring);
                        window.paint_path(ring.build().unwrap(), c(ACCENT()));
                    }
                },
            )
            .absolute()
            .inset_0(),
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
    if !models.is_null() {
        rows = rows.child(header_row());
    }
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
            .child(icon_for(&id))
            .child(row_copy(vstr(model, "name"), ""))
            .child(
                div()
                    .w(px(PIP_COL_W))
                    .flex_none()
                    .child(pips(vnum(model, "speedScore"))),
            )
            .child(
                div()
                    .w(px(PIP_COL_W))
                    .flex_none()
                    .child(pips(vnum(model, "accuracyScore"))),
            );
        if let Some(percent) = pill {
            r = r.child(progress_pill(&id, percent));
        } else if vbool(model, "downloaded") {
            r = r.child(delete_button(&id, cx));
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
