//! Интеграции — integrations.list/set_credential/clear_credential/sync_now +
//! login_contract (ConnectionsView.vue parity).
use ::gpui::{prelude::*, *};
use serde_json::{json, Value};

use crate::app::ManagerApp;
use crate::widgets::*;
use mundus_gpui_kit::theme::*;

pub fn load(app: &mut ManagerApp) {
    super::secrets::load(app);
    app.call("conn.list", "integrations.list", json!({}));
}

/// Input-state key shared by the field's creation and its submit handler —
/// KOS-279: the button read the bare provider id while the input lived under
/// `conn.cred.{id}`, so the value was always empty and nothing was sent.
fn cred_input_key(provider: &str, setting: &str) -> String {
    format!("conn.cred.{provider}.{setting}")
}

/// The only statuses the card can be in — derived from the snapshot flags, so
/// a raw enum string never reaches the screen.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum IntegrationState {
    /// Worker enabled — the integration is syncing.
    Connected,
    /// Credentials saved, worker not enabled yet.
    Configured,
    /// Nothing saved.
    NotConnected,
}

fn provider_state(p: &Value) -> IntegrationState {
    if vbool(p, "enabled") {
        IntegrationState::Connected
    } else if vbool(p, "hasCredential") {
        IntegrationState::Configured
    } else {
        IntegrationState::NotConnected
    }
}

/// Russian UI text for every state — exhaustive on purpose, a new state must
/// get its own wording instead of falling through to a raw enum name.
fn status_text(state: IntegrationState) -> &'static str {
    match state {
        IntegrationState::Connected => "Подключена",
        IntegrationState::Configured => "Данные сохранены",
        IntegrationState::NotConnected => "Не подключена",
    }
}

fn status_color(state: IntegrationState) -> u32 {
    match state {
        IntegrationState::Connected => SUCCESS(),
        IntegrationState::Configured => ACCENT(),
        IntegrationState::NotConnected => MUTED_FG(),
    }
}

/// Vault-bound credential kinds — the input is masked, the value goes to the
/// OS keyring via `integrations.set_credential`. Mirrors
/// `IntegrationSettingKind::is_secret` on the Engine side.
fn secret_kind(kind: &str) -> bool {
    matches!(kind, "secret" | "api_key" | "token")
}

/// Send every filled credential field for the provider. Each reply carries a
/// fresh `integrations.list` snapshot, so the card re-renders connected.
fn submit_credentials(
    this: &mut ManagerApp,
    provider: &str,
    settings: &[Value],
    cx: &Context<ManagerApp>,
) {
    for setting in settings {
        let key = vstr(setting, "key");
        let value = this.input_value(&cred_input_key(provider, &key), cx);
        if value.is_empty() {
            continue;
        }
        this.call(
            "conn.list",
            "integrations.set_credential",
            json!({
                "provider": provider,
                "setting": key,
                "kind": vstr(setting, "kind"),
                "credential": value,
            }),
        );
    }
}

pub fn render(
    app: &mut ManagerApp,
    window: &mut Window,
    cx: &mut Context<ManagerApp>,
) -> AnyElement {
    let mut col = page_stack();
    col = col.child(section("Интеграции", "Источники данных"));

    let snapshot = app.data("conn.list");
    let mut el = div().flex().flex_col().gap_3();
    let providers = varr(&snapshot, "providers").to_vec();
    if providers.is_empty() {
        let text = crate::async_fields::field_text(app.slots.get("conn.list"), |_| {
            "Нет доступных интеграций".into()
        });
        el = el.child(card().child(empty(&text)));
    }
    for p in &providers {
        el = el.child(render_provider(app, p, window, cx));
    }
    col = col.child(el);
    col.child(super::secrets::render_body(app, window, cx))
        .into_any_element()
}

fn render_provider(
    app: &mut ManagerApp,
    p: &Value,
    window: &mut Window,
    cx: &mut Context<ManagerApp>,
) -> AnyElement {
    let id = vstr(p, "id");
    // `label` is the manifest display name — required and validated by the
    // manifest schema, so a package id must never reach this screen.
    let name = vstr(p, "label");
    let state = provider_state(p);
    let browser_login = vstr(p, "authMode") == "browser_login";
    let settings = varr(p, "settingSchema").to_vec();
    let values = vget(p, "settingValues").clone();

    let mut card_el = card().child(
        entry_row(
            icon_file(vopt(p, "iconPath")),
            name.clone(),
            status_text(state).to_string(),
        )
        .child(badge(status_text(state), status_color(state))),
    );

    // Saved public values (ник) are plain config — shown back to the user,
    // never masked. Secrets stay in the vault and are only reported as saved.
    if state != IntegrationState::NotConnected {
        for setting in &settings {
            let (key, kind) = (vstr(setting, "key"), vstr(setting, "kind"));
            if secret_kind(&kind) {
                continue;
            }
            if let Some(value) = values.get(&key).and_then(Value::as_str) {
                if !value.is_empty() {
                    card_el = card_el.child(kv(&vstr(setting, "label"), value.to_string()));
                }
            }
        }
    }

    let mut actions = div()
        .w_full()
        .flex()
        .flex_wrap()
        .gap_2()
        .items_center()
        .justify_end();
    match state {
        IntegrationState::Connected => {
            let sid = id.clone();
            let did = id.clone();
            let dname = name.clone();
            actions = actions
                .child(
                    crate::button::primary(SharedString::from(format!("sync-{id}")))
                        .label("Синхронизировать")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.action(
                                "integrations.sync_now",
                                json!({"provider": sid, "provider_id": sid}),
                            );
                            cx.notify();
                        })),
                )
                .child(
                    crate::button::danger(SharedString::from(format!("disc-{id}")))
                        .label("Отключить")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.ask_confirm(
                                "Отключить интеграцию",
                                format!(
                                    "«{dname}» перестанет синхронизироваться, \
                                     сохранённые данные подключения будут удалены."
                                ),
                                "integrations.clear_credential",
                                json!({"provider": did, "provider_id": did}),
                                cx,
                            );
                        })),
                );
        }
        IntegrationState::Configured | IntegrationState::NotConnected => {
            if browser_login {
                let lid = id.clone();
                actions = actions.child(
                    crate::button::primary(SharedString::from(format!("login-{id}")))
                        .label("Подключить")
                        .on_click(cx.listener(move |this, _, _, _| {
                            this.call(
                                "conn.login",
                                "integrations.login_contract",
                                json!({"provider": lid, "provider_id": lid}),
                            );
                        })),
                );
            } else {
                for setting in &settings {
                    let (key, kind) = (vstr(setting, "key"), vstr(setting, "kind"));
                    let label = vstr(setting, "label");
                    let hint = vstr(setting, "description");
                    let input = app.input(
                        &cred_input_key(&id, &key),
                        if hint.is_empty() {
                            format!("{label}…")
                        } else {
                            hint.clone()
                        },
                        secret_kind(&kind),
                        window,
                        cx,
                    );
                    card_el = card_el.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(c(MUTED_FG()))
                                    .child(label.clone()),
                            )
                            .child(input_field(&input).aria_label(label).mask_toggle()),
                    );
                }
                let pid = id.clone();
                let schema = settings.clone();
                actions = actions.child(
                    crate::button::primary(SharedString::from(format!("save-{id}")))
                        .label("Подключить")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            submit_credentials(this, &pid, &schema, cx);
                        })),
                );
            }
            // Saved-but-disabled credentials can be dropped without a full
            // disconnect flow.
            if state == IntegrationState::Configured {
                let did = id.clone();
                actions = actions.child(
                    crate::button::ghost(SharedString::from(format!("clr-{id}")))
                        .label("Удалить данные")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.ask_confirm(
                                "Удалить данные подключения",
                                "Сохранённые учётные данные интеграции будут удалены.",
                                "integrations.clear_credential",
                                json!({"provider": did, "provider_id": did}),
                                cx,
                            );
                        })),
                );
            }
        }
    }
    card_el = card_el.child(actions);
    card_el.into_any_element()
}

#[cfg(test)]
mod tests {
    // No `use super::*` here: the parent glob-imports `gpui::*`, which
    // carries a `test` attribute macro that shadows the built-in `#[test]`
    // and blows the recursion limit.
    use super::{
        cred_input_key, provider_state, secret_kind, status_text, submit_credentials,
        IntegrationState,
    };
    use crate::app::{ManagerApp, Slot};
    use gpui::{AppContext, Entity, TestAppContext};
    use serde_json::json;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn status_text_covers_every_state() {
        // Exhaustive by construction — a new IntegrationState fails to
        // compile here, so it can never fall back to a raw enum name.
        assert_eq!(status_text(IntegrationState::Connected), "Подключена");
        assert_eq!(
            status_text(IntegrationState::Configured),
            "Данные сохранены"
        );
        assert_eq!(status_text(IntegrationState::NotConnected), "Не подключена");
        assert_eq!(
            provider_state(&json!({"enabled": true})),
            IntegrationState::Connected
        );
        assert_eq!(
            provider_state(&json!({"enabled": false, "hasCredential": true})),
            IntegrationState::Configured
        );
        assert_eq!(provider_state(&json!({})), IntegrationState::NotConnected);
    }

    #[test]
    fn secret_kinds_mask_and_public_kinds_do_not() {
        for kind in ["secret", "api_key", "token"] {
            assert!(secret_kind(kind), "{kind}");
        }
        for kind in ["username", "text"] {
            assert!(!secret_kind(kind), "{kind}");
        }
    }

    /// KOS-279 regression: the input is stored under `conn.cred.{id}.{key}`
    /// and the submit handler must read that exact key — before the fix it
    /// read the bare provider id, saw an empty value and sent nothing.
    #[gpui::test]
    async fn credential_save_reads_the_input_under_the_same_key(cx: &mut TestAppContext) {
        cx.update(gpui_component::init);
        cx.update(imago_gpui::theme::apply);
        let slot: Rc<RefCell<Option<Entity<ManagerApp>>>> = Rc::new(RefCell::new(None));
        let slot2 = slot.clone();
        let (_root, cx) = cx.add_window_view(move |window, cx| {
            let app = cx.new(|cx| ManagerApp::new(window, cx));
            *slot2.borrow_mut() = Some(app.clone());
            gpui_component::Root::new(app, window, cx)
        });
        let app = slot.borrow_mut().take().expect("window builder ran");
        let settings = vec![json!({"key": "username", "kind": "username", "label": "Ник"})];

        cx.update(|window, cx| {
            app.update(cx, |app, cx| {
                let input = app.input(
                    &cred_input_key("com.kosmos.codewars", "username"),
                    "Ник",
                    false,
                    window,
                    cx,
                );
                input.update(cx, |state, cx| state.set_value("toro", window, cx));
                submit_credentials(app, "com.kosmos.codewars", &settings, cx);
            });
        });

        app.read_with(cx, |app, _| {
            assert!(
                matches!(app.slots.get("conn.list"), Some(Slot::Loading)),
                "a typed credential must dispatch integrations.set_credential"
            );
        });
    }
}
