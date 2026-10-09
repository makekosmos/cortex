use crate::app::Slot;
use crate::views::{self, StoreTab, View, NAV_GROUPS};
use gpui::{px, size, TestAppContext};

#[test]
fn sidebar_contains_only_task_oriented_top_level_pages() {
    let pages: Vec<_> = NAV_GROUPS
        .iter()
        .flat_map(|group| group.iter().copied())
        .collect();
    assert_eq!(
        pages,
        vec![
            View::About,
            View::Appearance,
            View::Packages,
            View::Connections,
            View::Data,
            View::Usage,
            View::Sync,
            View::Browser,
            View::Keys,
            View::Models,
            View::Settings,
            View::Dev
        ]
    );
}

#[gpui::test]
fn relocated_controls_load_their_data_on_their_own_pages(cx: &mut TestAppContext) {
    let (manager, cx) = crate::a11y_tests::launch(cx);
    manager.update(cx, |app, _| {
        assert!(app.store_tab == StoreTab::Installed);
        app.slots.clear();
        views::settings::load(app);
        for slot in ["engine.autostart", "engine.settings"] {
            assert!(app.slots.contains_key(slot), "missing settings slot {slot}");
        }
        assert!(!app.slots.contains_key("browser.persist"));
        assert!(!app.slots.contains_key("backups.list"));
        assert!(!app.slots.contains_key("dev.packages"));
        views::browser::load(app);
        assert!(app.slots.contains_key("browser.persist"));
        views::dev::load(app);
        assert!(app.slots.contains_key("dev.packages"));
        views::data::load(app);
        assert!(app.slots.contains_key("backups.list"));
        views::connections::load(app);
        assert!(app.slots.contains_key("conn.list"));
        assert!(!app.slots.contains_key("secrets.config"));
        views::secrets::load(app);
        assert!(app.slots.contains_key("secrets.config"));
    });
}

#[gpui::test]
fn relocated_static_cards_remain_visible_when_values_are_pending_or_failed(
    cx: &mut TestAppContext,
) {
    let (manager, cx) = crate::a11y_tests::launch(cx);
    cx.simulate_resize(size(px(1440.), px(2000.)));
    for failed in [false, true] {
        manager.update(cx, |app, cx| {
            app.view = View::Settings;
            for slot in ["engine.autostart", "engine.settings"] {
                app.slots.insert(
                    slot.into(),
                    if failed {
                        Slot::Failed("offline".into())
                    } else {
                        Slot::Loading
                    },
                );
            }
            cx.notify();
        });
        cx.update(|_, cx| cx.refresh_windows());
        assert!(
            cx.debug_bounds("settings-startup-card").is_some(),
            "missing static card settings-startup-card"
        );
        manager.update(cx, |app, cx| {
            app.view = View::Data;
            app.slots.insert(
                "backups.list".into(),
                if failed {
                    Slot::Failed("offline".into())
                } else {
                    Slot::Loading
                },
            );
            cx.notify();
        });
        cx.update(|_, cx| cx.refresh_windows());
        assert!(cx.debug_bounds("data-backups-card").is_some());
        manager.update(cx, |app, cx| {
            app.view = View::Keys;
            cx.notify();
        });
        cx.update(|_, cx| cx.refresh_windows());
        assert!(cx.debug_bounds("integration-access-card").is_some());
        assert!(cx.debug_bounds("key-company-openai").is_some());
        assert!(cx.debug_bounds("key-company-nvidia").is_some());
    }
}

#[test]
fn saved_updates_page_redirects_to_about() {
    assert_eq!(View::from_key("updates"), Some(View::About));
}

/// KOS-368: on macOS the Dock's window list showed two entries for the
/// single Manager window. Root cause: gpui-pre-macos `set_title` calls
/// `changeWindowsItem`, which *adds* the NSWindow to the Dock/Window list
/// when AppKit has not tracked it yet; running it during `open_window`
/// (before order-in) left a phantom entry once AppKit added the window
/// itself. The fix defers the title until after the window is ordered in:
/// creation options must not carry a title, and the real startup path must
/// still open exactly one window.
#[gpui::test]
fn startup_window_defers_title_and_opens_exactly_one_window(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    cx.update(imago_gpui::theme::apply);
    let windows = cx.update(|cx| {
        let options = crate::manager_window_options(cx);
        assert!(
            options
                .titlebar
                .as_ref()
                .and_then(|t| t.title.as_ref())
                .is_none(),
            "creation-time window title reintroduces the phantom Dock entry"
        );
        crate::open_manager_window(cx);
        cx.windows()
    });
    assert_eq!(windows.len(), 1, "expected one window, got {windows:?}");
    let mut window_cx = gpui::VisualTestContext::from_window(windows[0], cx);
    assert_eq!(window_cx.window_title().as_deref(), Some("Mundus"));
}
