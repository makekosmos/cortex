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
            View::Packages,
            View::Data,
            View::Usage,
            View::Sync,
            View::Connections,
            View::Keys,
            View::Settings,
            View::Appearance,
            View::Browser,
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
