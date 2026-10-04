//! Window-independent app actions, without adding controls to the titlebar.
use crate::app::ManagerApp;
use gpui::{actions, App, KeyBinding, WeakEntity};

actions!(manager, [Refresh]);

pub fn register(cx: &mut App, manager: WeakEntity<ManagerApp>) {
    cx.on_action(move |_: &Refresh, cx| {
        let _ = manager.update(cx, |app, cx| {
            app.load_current();
            cx.notify();
        });
    });
    cx.bind_keys([KeyBinding::new(
        if cfg!(target_os = "macos") {
            "cmd-r"
        } else {
            "ctrl-r"
        },
        Refresh,
        None,
    )]);
}

#[cfg(test)]
mod tests {
    use super::{register, Refresh};
    use crate::app::Slot;
    use gpui::{Keystroke, TestAppContext};
    use serde_json::json;

    #[gpui::test]
    fn keyboard_refresh_reloads_cached_page(cx: &mut TestAppContext) {
        let (manager, cx) = crate::a11y_tests::launch(cx);
        cx.update(|_, cx| register(cx, manager.downgrade()));
        manager.update(cx, |app, _| {
            app.view = crate::views::View::Data;
            app.slots
                .insert("data.summary".into(), Slot::Ready(json!({})));
        });
        let key = if cfg!(target_os = "macos") {
            "cmd-r"
        } else {
            "ctrl-r"
        };
        cx.update(|window, _| {
            assert!(window.bindings_for_action(&Refresh).iter().any(|binding| {
                binding.match_keystrokes(&[Keystroke::parse(key).unwrap()]) == Some(false)
            }));
        });
        cx.simulate_keystrokes(key);
        manager.read_with(cx, |app, _| {
            assert!(matches!(app.slots.get("data.summary"), Some(Slot::Loading)));
        });
    }
}
