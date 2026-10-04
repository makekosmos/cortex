//! Company keys in one settings card. The row opens a modal; Check becomes
//! Save only after that exact key is accepted.
use ::gpui::{prelude::*, AnimationExt, *};
use serde_json::json;
use std::time::Duration;

use crate::app::{ManagerApp, Slot};
use crate::theme::*;
use crate::widgets::*;

struct Company {
    id: &'static str,
    name: &'static str,
    icon: &'static str,
    stores_key: bool,
}

const COMPANIES: &[Company] = &[
    Company {
        id: "groq",
        name: "Groq",
        icon: "icons/providers/groq.svg",
        stores_key: true,
    },
    Company {
        id: "openai",
        name: "OpenAI",
        icon: "icons/providers/openai.svg",
        stores_key: false,
    },
    Company {
        id: "nvidia",
        name: "NVIDIA",
        icon: "icons/providers/nvidia.svg",
        stores_key: false,
    },
];

pub fn load(app: &mut ManagerApp) {
    app.call("secrets.config", "dictation.get_config", json!({}));
}

pub fn render(
    app: &mut ManagerApp,
    _window: &mut Window,
    cx: &mut Context<ManagerApp>,
) -> AnyElement {
    let mut rows = card()
        .id("keys-card")
        .debug_selector(|| "keys-card".into())
        .px(px(0.))
        .py(px(0.))
        .gap(px(0.));
    for (index, company) in COMPANIES.iter().enumerate() {
        rows = rows.child(company_row(app, company, index == 0, cx));
    }
    page_sections().child(rows).into_any_element()
}

fn company_row(
    _app: &mut ManagerApp,
    company: &Company,
    first: bool,
    cx: &mut Context<ManagerApp>,
) -> Stateful<Div> {
    let id = company.id;
    div()
        .id(SharedString::from(format!("key-company-{id}")))
        .debug_selector(move || {
            if id == "groq" {
                "integration-access-card".into()
            } else {
                format!("key-company-{id}")
            }
        })
        .mx(px(16.))
        .py(px(12.))
        .min_h(px(52.))
        .when(!first, |row| {
            row.border_t_1().border_color(fade(BORDER(), 0.6))
        })
        .flex()
        .items_center()
        .gap(px(12.))
        .when(company.stores_key, |row| {
            row.cursor_pointer()
                .role(Role::Button)
                .aria_label(format!("Ключ {}", company.name))
                .on_click(cx.listener(move |app, _, _, cx| open_editor(app, id, cx)))
        })
        .when(!company.stores_key, |row| row.opacity(0.4))
        .child(
            svg()
                .path(company.icon)
                .size(px(18.))
                .flex_none()
                .text_color(c(FG())),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(ui_px(13.))
                .line_height(ui_px(17.))
                .font_weight(FontWeight::MEDIUM)
                .child(company.name),
        )
        .when(company.stores_key, |row| {
            row.child(
                svg()
                    .path("icons/alt-arrow-right.svg")
                    .size(px(16.))
                    .flex_none()
                    .text_color(c(MUTED_FG())),
            )
        })
}

fn open_editor(app: &mut ManagerApp, id: &'static str, cx: &mut Context<ManagerApp>) {
    app.slots.remove("secrets.verify");
    app.key_checked = None;
    app.key_editor = Some(id.to_owned());
    cx.notify();
}

pub fn render_modal(
    app: &mut ManagerApp,
    window: &mut Window,
    cx: &mut Context<ManagerApp>,
) -> AnyElement {
    let Some(id) = app.key_editor.clone() else {
        return div().into_any_element();
    };
    let Some(company) = COMPANIES.iter().find(|company| company.id == id) else {
        return div().into_any_element();
    };
    let key_in = app.input("secrets.key", "Вставьте ключ", true, window, cx);
    let typed = app.input_value("secrets.key", cx);
    let checking = matches!(app.slots.get("secrets.verify"), Some(Slot::Loading));
    let verified = company.stores_key
        && app.key_checked.as_deref() == Some(typed.as_str())
        && !typed.is_empty()
        && vbool(&app.data("secrets.verify"), "valid");
    let failure = if company.stores_key
        && app.key_checked.as_deref() == Some(typed.as_str())
        && matches!(app.slots.get("secrets.verify"), Some(Slot::Ready(_)))
        && !verified
    {
        vopt(&app.data("secrets.verify"), "message").unwrap_or_else(|| "Ключ отклонён.".into())
    } else {
        String::new()
    };
    let body = div()
        .id("key-modal-scrim")
        .absolute()
        .size_full()
        .bg(fade(0x000000, 0.5))
        .flex()
        .items_center()
        .justify_center()
        .occlude()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(cx.listener(|app, _, _, cx| {
            // Close on release, after this layer has already taken the press.
            // Mouse-down removal lets the release activate a control behind.
            app.key_editor = None;
            cx.stop_propagation();
            cx.notify();
        }))
        .child(
            div()
                .id("key-modal-card")
                .w_full()
                .max_w(px(420.))
                .mx(px(24.))
                .p(px(16.))
                .rounded(px(12.))
                .bg(c(POPOVER()))
                .flex()
                .flex_col()
                .gap(px(12.))
                .occlude()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(|_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(svg().path(company.icon).size(px(18.)).text_color(c(FG())))
                        .child(
                            div()
                                .text_size(ui_px(15.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(company.name),
                        ),
                )
                .child(input_field(&key_in).aria_label(format!("API-ключ {}", company.name)))
                .when(!company.stores_key, |body| {
                    body.child(empty("Хранение ключа этой компании ещё не подключено."))
                })
                .when(!failure.is_empty(), |body| body.child(empty(&failure)))
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .gap(px(8.))
                        .child(
                            crate::button::ghost("key-cancel")
                                .label("Закрыть")
                                .on_click(cx.listener(|app, _, _, cx| {
                                    cx.stop_propagation();
                                    app.key_editor = None;
                                    cx.notify();
                                })),
                        )
                        .when(company.stores_key, |row| {
                            row.child(action_button(app, verified, checking, cx))
                        }),
                ),
        );
    body.into_any_element()
}

fn action_button(
    _app: &ManagerApp,
    verified: bool,
    checking: bool,
    cx: &mut Context<ManagerApp>,
) -> impl IntoElement {
    let label = if checking {
        "Проверяем…"
    } else if verified {
        "Сохранить"
    } else {
        "Проверить"
    };
    let button = if verified {
        crate::button::primary("secrets-save")
    } else {
        crate::button::secondary("secrets-verify")
    }
    .label(label)
    .disabled(checking)
    .on_click(cx.listener(move |app, _, _, cx| {
        let key = app.input_value("secrets.key", cx);
        if key.is_empty() {
            return;
        }
        if verified {
            app.action("dictation.set_api_key", json!({"key": key}));
            app.key_editor = None;
            app.key_checked = None;
        } else {
            app.key_checked = Some(key.clone());
            app.slots.insert("secrets.verify".into(), Slot::Loading);
            app.call(
                "secrets.verify",
                "dictation.verify_api_key",
                json!({"key": key}),
            );
        }
        cx.notify();
    }));
    if !verified {
        return button.into_any_element();
    }
    div()
        .with_animation(
            "key-save-reveal",
            Animation::new(Duration::from_millis(180)).with_easing(|t| 1. - (1. - t).powi(3)),
            |element, progress| element.opacity(0.4 + 0.6 * progress),
        )
        .child(button)
        .into_any_element()
}
