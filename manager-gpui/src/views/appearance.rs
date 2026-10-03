//! Persistent appearance editor. Engine owns policy; controls never infer OS support.
use crate::app::{ManagerApp, Slot};
use crate::theme::*;
use crate::widgets::*;
use ::gpui::{prelude::*, *};
use gpui_component::Disableable;
use serde_json::{json, Value};

mod colors;
mod previews;
mod typography;

pub fn load(app: &mut ManagerApp) {
    app.call("appearance", "appearance.get", json!({}));
}

pub(super) fn editable(app: &ManagerApp) -> bool {
    !app.action_busy
        && app.appearance.ready
        && matches!(app.slots.get("appearance"), Some(Slot::Ready(_)))
}

pub(super) fn patch(app: &mut ManagerApp, params: Value, cx: &mut Context<ManagerApp>) {
    if editable(app) {
        app.action("appearance.set", params);
        cx.notify();
    }
}

pub(super) fn choice(
    app: &ManagerApp,
    id: String,
    label: &str,
    selected: bool,
    params: Value,
    cx: &mut Context<ManagerApp>,
) -> crate::button::Button {
    crate::button::button(
        SharedString::from(id),
        if selected {
            crate::button::ButtonKind::Primary
        } else {
            crate::button::ButtonKind::Ghost
        },
    )
    .label(label.to_owned())
    .disabled(!editable(app))
    .on_click(cx.listener(move |app, _, _, cx| patch(app, params.clone(), cx)))
}

pub(super) fn watched_input(
    app: &mut ManagerApp,
    key: &str,
    placeholder: &str,
    window: &mut Window,
    cx: &mut Context<ManagerApp>,
) -> Entity<gpui_component::input::InputState> {
    let new = !app.inputs.contains_key(key);
    let input = app.input(key, placeholder.to_owned(), false, window, cx);
    if new {
        cx.subscribe(&input, |_, _, _: &gpui_component::input::InputEvent, cx| {
            cx.notify()
        })
        .detach();
    }
    input
}

pub fn render(
    app: &mut ManagerApp,
    window: &mut Window,
    cx: &mut Context<ManagerApp>,
) -> AnyElement {
    let status = match app.slots.get("appearance") {
        Some(Slot::Ready(_)) => "",
        Some(Slot::Failed(_)) => {
            "Настройки внешнего вида недоступны. Сохранённый стиль не изменён."
        }
        _ => "Загрузка настроек внешнего вида…",
    };
    let mut body = page_sections().child(section(
        "Внешний вид",
        "Темы, цвет, материалы и типографика",
    ));
    if !status.is_empty() {
        body = body.child(empty(status));
    }
    body = body
        .child(previews::modes(app, cx))
        .child(previews::palettes(app, cx))
        .child(colors::render(app, window, cx));
    let enabled = editable(app);
    let follow = app.appearance.settings.follow_apps;
    let follow_control = if app.appearance.ready {
        toggle("appearance-follow-apps", follow, cx, |app, follow, cx| {
            patch(app, json!({"follow_apps":follow}), cx);
        })
        .accessibility_label("Единый стиль приложений")
        .disabled(!enabled)
        .into_any_element()
    } else {
        badge(
            if matches!(app.slots.get("appearance"), Some(Slot::Failed(_))) {
                "Недоступно"
            } else {
                "Загрузка…"
            },
            MUTED_FG(),
        )
        .into_any_element()
    };
    body = body.child(
        section_group()
            .child(section("Приложения", "Одна настройка — общий стиль"))
            .child(
                card()
                    .id("appearance-apps-card")
                    .debug_selector(|| "appearance-apps-card".into())
                    .child(
                        row(
                            "Единый стиль приложений",
                            "Приложения используют этот стиль. Выключите для отдельных настроек.",
                        )
                        .child(follow_control),
                    )
                    .child(empty(
                        "Agenda поддерживает общий стиль. Остальным клиентам нужен API Engine.",
                    )),
            ),
    );
    let mut materials = div().flex().flex_wrap().gap(px(8.));
    for (key, label) in [
        ("default", "Цвета темы"),
        ("frosted", "Матовое стекло"),
        ("opaque", "Непрозрачный"),
        ("acrylic", "Acrylic"),
        ("mica", "Mica"),
    ] {
        if app.appearance.materials.iter().any(|item| item == key) {
            materials = materials.child(choice(
                app,
                format!("material-{key}"),
                label,
                app.appearance.ready && app.appearance.settings.material == key,
                json!({"material":key}),
                cx,
            ));
        }
    }
    body.child(
        section_group()
            .child(section("Материал", "Доступные эффекты сообщает Engine"))
            .child(
                card()
                    .id("appearance-material-card")
                    .debug_selector(|| "appearance-material-card".into())
                    .child(materials)
                    .child(empty("Нативный фон окна; карточки остаются непрозрачными.")),
            ),
    )
    .child(typography::render(app, window, cx))
    .into_any_element()
}
