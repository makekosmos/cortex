//! Разработка — dev environment info + packages.install_development
//! (DevPanelView.vue parity). Dev package discovery stays Host-owned
//! (dev-packages.json is repo-local); GPUI installs by explicit path.
use ::gpui::{prelude::*, *};
use gpui_component::input::Input;
use serde_json::json;

use crate::app::ManagerApp;
use crate::theme::*;
use crate::widgets::*;

pub fn load(app: &mut ManagerApp) {
    app.call("dev.packages", "packages.list", json!({}));
}

pub fn render(
    app: &mut ManagerApp,
    window: &mut Window,
    cx: &mut Context<ManagerApp>,
) -> AnyElement {
    let mut col = div().flex().flex_col().gap_4().w_full();
    col = col.child(section("Разработка", "Локальные пакеты и инстанс"));

    col = col.child(
        card()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Инстанс"),
            )
            .child(kv("Режим", "GPUI Manager"))
            .child(kv(
                "Данные",
                crate::engine::data_dir()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|_| "не найдена".into()),
            ))
            .child(kv(
                "Dev-пакеты",
                "discovery через dev-packages.json — Host; установка по пути доступна",
            )),
    );

    let path_in = app.input(
        "dev.path",
        "Путь к release/*.kspkg или папке пакета…",
        window,
        cx,
    );
    col = col.child(
        card()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Установить dev-пакет"),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(div().flex_1().child(Input::new(&path_in)))
                    .child(btn(
                        "dev-install",
                        "Установить",
                        true,
                        cx,
                        |this, cx| {
                            let p = this.input_value("dev.path", cx);
                            if p.is_empty() {
                                return;
                            }
                            match crate::devpkg::resolve(std::path::Path::new(&p)) {
                                Ok(t) => this.action(
                                    "packages.install_development",
                                    json!({
                                        "package_id": t.id,
                                        "version": t.version,
                                        "archive_path": t.archive_path.to_string_lossy(),
                                    }),
                                ),
                                Err(e) => this.error = Some(e),
                            }
                            cx.notify();
                        },
                    )),
            ),
    );

    col = col.child(slot_or(app, "dev.packages", |v| {
        let items = varr(v, "packages");
        let dev: Vec<_> = items
            .iter()
            .filter(|p| {
                vstr(p, "source") == "development"
                    || vbool(p, "development")
                    || vstr(p, "channel") == "dev"
            })
            .cloned()
            .collect();
        let mut el = card();
        el = el.child(
            div()
                .text_xs()
                .text_color(c(MUTED_FG))
                .child(format!("Dev-пакеты ({})", dev.len())),
        );
        if dev.is_empty() {
            el = el.child(empty("Dev-пакеты не установлены"));
        }
        for p in dev {
            let id = vstr(&p, "id");
            let uid = id.clone();
            el = el.child(
                row(
                    vopt(&p, "name").unwrap_or_else(|| id.clone()),
                    format!("v{} · {}", vstr(&p, "version"), id),
                )
                .child(btn_id(&format!("dev-un-{id}"), "Удалить", {
                    cx.listener(move |this, _, _, cx| {
                        this.ask_confirm(
                            "Удалить dev-пакет",
                            "Пакет будет удалён из Engine.",
                            "packages.uninstall",
                            json!({"package_id": uid}),
                            cx,
                        );
                    })
                })),
            );
        }
        el.into_any_element()
    }));
    col.into_any_element()
}
