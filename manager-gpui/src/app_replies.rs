//! Reply handlers for side slots whose answers are not view data: a launch
//! URL to open, a login page to hand to the system browser, a marketplace
//! link. Split from `app.rs` to keep that file under the source-size gate.
use serde_json::Value;

use crate::app::ManagerApp;

impl ManagerApp {
    /// Hand an Engine-provided URL to the system browser, or raise `missing`
    /// when the reply carried none; an Engine error is shown as is.
    fn open_reply_url(
        &mut self,
        result: Result<Value, String>,
        url_of: impl FnOnce(&Value) -> Option<&str>,
        missing: &str,
    ) {
        match result {
            Ok(v) => match url_of(&v).filter(|url| !url.is_empty()) {
                Some(url) => {
                    if let Err(e) = mundus_gpui_kit::engine::open_url(url) {
                        self.error = Some(e);
                    }
                }
                None => self.error = Some(missing.into()),
            },
            Err(e) => self.error = Some(e),
        }
    }

    /// The `pkg.open` reply: open the launch URL or surface the typed
    /// Engine error (already a Russian line — `worker::package_open_message`).
    pub(crate) fn open_reply(&mut self, result: Result<Value, String>) {
        self.open_reply_url(
            result,
            |v| v.get("launch_url")?.as_str(),
            "Engine не вернул адрес приложения.",
        );
    }

    /// The `@action` reply: a mutation finished — reload the current view.
    /// `connect_with_pairing_code` answers `{"status":"connected", …}` once
    /// the peer's Hello completed, so the notice names the device instead of
    /// the generic «Выполнено.»; an Engine error goes to the banner
    /// (KOS-367 — pairing must never fail silently).
    pub(crate) fn action_reply(&mut self, result: Result<Value, String>) {
        self.action_busy = false;
        match result {
            Ok(value) => {
                let peer_name = (value.get("status").and_then(Value::as_str) == Some("connected"))
                    .then(|| value.get("device_name").and_then(Value::as_str))
                    .flatten()
                    .map(str::trim)
                    .filter(|name| !name.is_empty())
                    .map(str::to_owned);
                self.notice = Some(match peer_name {
                    Some(name) => format!("Устройство подключено: {name}."),
                    None => "Выполнено.".into(),
                });
                self.reload_current_view();
            }
            Err(e) => self.error = Some(e),
        }
    }

    /// The `conn.login` reply: `integrations.login_contract` hands back the
    /// provider's login page — open it in the system browser.
    pub(crate) fn login_reply(&mut self, result: Result<Value, String>) {
        self.open_reply_url(
            result,
            |v| v.pointer("/login/startUrl")?.as_str(),
            "Интеграция не вернула страницу входа.",
        );
    }

    /// The `store.ext` reply: `store.external_url` → open in the system
    /// browser. The answer is `{url}` or a bare string.
    pub(crate) fn external_url_reply(&mut self, result: Result<Value, String>) {
        self.open_reply_url(
            result,
            |v| v.get("url").and_then(Value::as_str).or_else(|| v.as_str()),
            "Engine не вернул ссылку маркетплейса.",
        );
    }
}
