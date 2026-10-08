//! Разработка — dev environment info + packages.install_development
//! (DevPanelView.vue parity). Dev package discovery stays Host-owned
//! (dev-packages.json is repo-local); GPUI installs by explicit path.
use ::gpui::{prelude::*, *};
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
    page_sections()
        .child(render_tools(app, window, cx))
        .into_any_element()
}

pub fn render_tools(
    app: &mut ManagerApp,
    window: &mut Window,
    cx: &mut Context<ManagerApp>,
) -> AnyElement {
    let mut col = page_stack();
    col = col.child(
        card().child(
            row(
                "FPS-счётчик",
                "График, средний FPS, 1% и 0.1% low в правом нижнем углу.",
            )
            .child(
                toggle("dev-fps", app.dev_fps, cx, |this, checked, _| {
                    this.dev_fps = checked;
                })
                .accessibility_label("FPS-счётчик"),
            ),
        ),
    );

    col = col.child(
        card()
            .child(
                div()
                    .text_size(crate::theme::ui_px(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Инстанс"),
            )
            .child(kv("Режим", "GPUI Manager"))
            .child(kv(
                "Данные",
                mundus_gpui_kit::engine::data_dir()
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
        false,
        window,
        cx,
    );
    col = col.child(
        card()
            .child(
                div()
                    .text_size(crate::theme::ui_px(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Установить dev-пакет"),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(div().flex_1().min_w_0().child(input_field(&path_in)))
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
        // Dev installs never come from the signed catalog — catalog_sequence
        // 0 is the marker (see packages.install_development).
        let dev: Vec<_> = items
            .iter()
            .filter(|p| super::store::is_development(p))
            .cloned()
            .collect();
        let mut el = card();
        el = el.child(
            div()
                .text_size(crate::theme::ui_px(12.))
                .text_color(c(MUTED_FG()))
                .child(format!("Dev-пакеты ({})", dev.len())),
        );
        if dev.is_empty() {
            el = el.child(empty("Dev-пакеты не установлены"));
        }
        for p in dev {
            let id = vstr(&p, "id");
            let uid = id.clone();
            let uversion = vstr(&p, "version");
            el = el.child(
                entry_row(
                    icon_file(vopt(&p, "icon_path")),
                    // The manifest name is required; the package id stays an
                    // internal key, never a caption (KOS-279).
                    vopt(&p, "name").unwrap_or_else(|| "Dev-пакет".into()),
                    format!("v{}", vstr(&p, "version")),
                )
                .child(btn_id(&format!("dev-un-{id}"), "Удалить", {
                    cx.listener(move |this, _, _, cx| {
                        this.ask_confirm(
                            "Удалить dev-пакет",
                            "Пакет будет удалён из Engine.",
                            "packages.uninstall",
                            json!({"package_id": uid, "version": uversion}),
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
