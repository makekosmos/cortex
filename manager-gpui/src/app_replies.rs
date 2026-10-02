//! Reply handlers for side slots whose answers are not view data: a launch
//! URL to open, a login page to hand to the system browser, a marketplace
//! link. Split from `app.rs` to keep that file under the source-size gate.
use serde_json::Value;

use crate::app::ManagerApp;

impl ManagerApp {
    /// The `pkg.open` reply: open the launch URL or surface the typed
    /// Engine error (already a Russian line — `worker::package_open_message`).
    pub(crate) fn open_reply(&mut self, result: Result<Value, String>) {
        match result {
            Ok(v) => match v.get("launch_url").and_then(Value::as_str) {
                Some(url) if !url.is_empty() => {
                    if let Err(e) = mundus_gpui_kit::engine::open_url(url) {
                        self.error = Some(e);
                    }
                }
                _ => self.error = Some("Engine не вернул адрес приложения.".into()),
            },
            Err(e) => self.error = Some(e),
        }
    }

    /// The `conn.login` reply: `integrations.login_contract` hands back the
    /// provider's login page — open it in the system browser.
    pub(crate) fn login_reply(&mut self, result: Result<Value, String>) {
        match result {
            Ok(v) => match v.pointer("/login/startUrl").and_then(Value::as_str) {
                Some(url) if !url.is_empty() => {
                    if let Err(e) = mundus_gpui_kit::engine::open_url(url) {
                        self.error = Some(e);
                    }
                }
                _ => self.error = Some("Интеграция не вернула страницу входа.".into()),
            },
            Err(e) => self.error = Some(e),
        }
    }

    /// The `store.ext` reply: `store.external_url` → open in the system
    /// browser.
    pub(crate) fn external_url_reply(&mut self, result: Result<Value, String>) {
        match result {
            Ok(v) => {
                let url = v
                    .get("url")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| v.as_str().unwrap_or_default().to_string());
                if url.is_empty() {
                    self.error = Some("Engine не вернул ссылку маркетплейса.".into());
                } else if let Err(e) = mundus_gpui_kit::engine::open_url(&url) {
                    self.error = Some(e);
                }
            }
            Err(e) => self.error = Some(e),
        }
    }
}
