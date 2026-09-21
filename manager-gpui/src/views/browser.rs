//! Браузер — browser.json persistence toggle. The Vue Manager owns the file
//! inside Electron userData ("Kosmos Manager"); GPUI reads/writes the same
//! schema so both shells agree (BrowserSettingsView.vue parity).
use ::gpui::{prelude::*, *};

use crate::app::ManagerApp;
use crate::theme::*;
use crate::widgets::*;

pub fn load(app: &mut ManagerApp) {
    app.slots.insert(
        "browser.persist".into(),
        crate::app::Slot::Ready(serde_json::json!({"value": read_persist()})),
    );
}

fn path() -> Option<std::path::PathBuf> {
    crate::engine::host_user_data().map(|d| d.join("browser.json"))
}

fn read_persist() -> bool {
    path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v.get("persistData").and_then(|b| b.as_bool()))
        .unwrap_or(true)
}

fn write_persist(enabled: bool) -> Result<(), String> {
    let p = path().ok_or("Не найдена папка данных Host")?;
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(&p, serde_json::json!({"persistData": enabled}).to_string())
        .map_err(|e| format!("Не удалось записать browser.json: {e}"))
}

pub fn render(
    app: &mut ManagerApp,
    _window: &mut Window,
    cx: &mut Context<ManagerApp>,
) -> AnyElement {
    let mut col = div().flex().flex_col().gap_4().w_full();
    col = col.child(section("Браузер", "Сессии и данные сайтов"));

    let persist = vbool(&app.data("browser.persist"), "value");
    col = col.child(
        card()
            .child(
                row(
                    "Сохранять данные сайтов",
                    "Куки и сессии встроенного браузера между запусками.",
                )
                .child(toggle(
                    "browser-persist",
                    persist,
                    cx,
                    |this, checked, cx| {
                        match write_persist(checked) {
                            Ok(()) => {
                                this.slots.insert(
                                    "browser.persist".into(),
                                    crate::app::Slot::Ready(serde_json::json!({"value": checked})),
                                );
                                this.notice = Some("Выполнено.".into());
                            }
                            Err(e) => this.error = Some(e),
                        }
                        cx.notify();
                    },
                )),
            )
            .child(div().text_xs().text_color(c(MUTED_FG)).child(format!(
                "Файл: {}",
                path().map(|p| p.display().to_string()).unwrap_or_default()
            ))),
    );
    col.into_any_element()
}
