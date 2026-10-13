use super::selected_categories;
use crate::{app::Slot, views::View};
use gpui::TestAppContext;
use serde_json::json;

#[test]
fn sorting_is_descending_and_equal_sizes_have_a_stable_order() {
    let mut first = json!({"categories":[
        {"id":"z", "bytes":10, "detail":[{"label":"small", "bytes":1}, {"label":"large", "bytes":5.5}]},
        {"id":"large", "bytes":20}, {"id":"a", "bytes":10}
    ]});
    let mut second = first.clone();
    second["categories"].as_array_mut().unwrap().reverse();
    super::normalize(&mut first);
    super::normalize(&mut second);
    assert_eq!(first, second);
    let items = first["categories"].as_array().unwrap();
    assert_eq!(items[0]["id"], "large");
    assert_eq!(items[1]["id"], "a");
    assert_eq!(items[2]["detail"][0]["label"], "large");
}

#[gpui::test]
fn empty_database_parts_are_plain_rows_and_refresh_button_is_absent(cx: &mut TestAppContext) {
    let (manager, cx) = crate::a11y_tests::launch(cx);
    manager.update(cx, |app, cx| {
        app.view = View::Storage;
        app.storage_database_only = true;
        app.slots.insert("data.storage".into(), Slot::Ready(json!({"categories":[
            {"id":"database", "bytes":4096, "detail":[{"id":"objects", "label":"Объекты", "bytes":4096}]}
        ]})));
        cx.notify();
    });
    cx.update(|_, cx| cx.refresh_windows());
    let row = cx.debug_bounds("storage-toggle-objects").unwrap();
    cx.simulate_click(row.center(), Default::default());
    manager.read_with(cx, |app, _| assert!(app.storage_ui.open.is_empty()));
    let tree = cx
        .update(|window, _| window.debug_a11y_tree_json())
        .unwrap();
    assert!(!tree.contains("storage-refresh"));
    assert!(!tree.contains("\"label\":\"Объекты\""));
}

#[gpui::test]
fn scope_tabs_change_visible_categories(cx: &mut TestAppContext) {
    let (manager, cx) = crate::a11y_tests::launch(cx);
    manager.update(cx, |app, cx| {
        app.view = View::Storage;
        app.slots.insert(
            "data.storage".into(),
            Slot::Ready(json!({
                "categories": [
                    {"id":"database", "label":"База данных", "bytes":4096},
                    {"id":"cache", "label":"Кеш", "bytes":1024}
                ]
            })),
        );
        cx.notify();
    });
    cx.update(|_, cx| cx.refresh_windows());
    let tab = cx.debug_bounds("storage-database").unwrap();
    cx.simulate_click(tab.center(), Default::default());
    cx.update(|_, cx| cx.refresh_windows());
    manager.read_with(cx, |app, _| assert!(app.storage_database_only));
    assert!(cx.debug_bounds("storage-category-cache").is_none());
    let tab = cx.debug_bounds("storage-all").unwrap();
    cx.simulate_click(tab.center(), Default::default());
    cx.update(|_, cx| cx.refresh_windows());
    manager.read_with(cx, |app, _| assert!(!app.storage_database_only));
    assert!(cx.debug_bounds("storage-category-cache").is_some());
}

#[test]
fn database_filter_excludes_installations_backups_and_cache() {
    let value = json!({"categories": [
        {"id":"database", "bytes":4096},
        {"id":"backups", "bytes":2048},
        {"id":"installations", "bytes":8192},
        {"id":"cache", "bytes":512}
    ]});
    assert_eq!(selected_categories(&value, false).len(), 4);
    let database = selected_categories(&value, true);
    assert_eq!(database.len(), 1);
    assert_eq!(database[0]["bytes"], 4096);
}

#[gpui::test]
fn categories_start_closed_and_bar_click_opens_the_matching_category(cx: &mut TestAppContext) {
    let (manager, cx) = crate::a11y_tests::launch(cx);
    manager.update(cx, |app, cx| {
        app.view = View::Storage;
        app.slots.insert("data.storage".into(), Slot::Ready(json!({"categories":[
            {"id":"database", "label":"База данных", "bytes":4096},
            {"id":"dictation_models", "label":"Модели диктовки", "bytes":64000,
             "description":"Локальное распознавание речи",
             "detail":[{"label":"Parakeet Ultra", "description":"Речь в текст на компьютере", "bytes":64000}]}
        ]})));
        cx.notify();
    });
    cx.update(|_, cx| cx.refresh_windows());
    manager.read_with(cx, |app, _| assert!(app.storage_ui.open.is_empty()));
    let segment = cx.debug_bounds("storage-segment-dictation_models").unwrap();
    cx.simulate_click(segment.center(), Default::default());
    cx.update(|_, cx| cx.refresh_windows());
    manager.read_with(cx, |app, _| {
        assert!(app.storage_ui.open.contains("dictation_models"));
        assert!(!app.storage_ui.open.contains("database"));
    });
    let row = cx.debug_bounds("storage-toggle-dictation_models").unwrap();
    cx.simulate_click(row.center(), Default::default());
    cx.update(|_, cx| cx.refresh_windows());
    manager.read_with(cx, |app, _| {
        assert!(!app.storage_ui.open.contains("dictation_models"))
    });
}

#[gpui::test]
fn graph_fills_width_and_record_breakdown_opens_on_click(cx: &mut TestAppContext) {
    let (manager, cx) = crate::a11y_tests::launch(cx);
    manager.update(cx, |app, cx| {
        app.view = View::Storage;
        app.storage_database_only = true;
        app.slots.insert(
            "data.storage".into(),
            Slot::Ready(json!({"categories":[
                {"id":"database", "bytes":1234567, "detail":[
                    {"id":"objects", "label":"Задачи, проекты и другие записи", "bytes":1234566,
                     "detail":[{"label":"Содержимое записей", "bytes":1000000},
                               {"label":"Индексы", "bytes":234566}]},
                    {"id":"schema", "label":"Структура базы", "bytes":1}
                ]}
            ]})),
        );
        cx.notify();
    });
    cx.update(|_, cx| cx.refresh_windows());
    let bar = cx.debug_bounds("storage-bar").unwrap();
    let last = cx.debug_bounds("storage-segment-schema").unwrap();
    assert!((last.right() - bar.right()).abs() < gpui::px(1.));
    manager.read_with(cx, |app, _| {
        assert!(!app.storage_ui.open.contains("objects"))
    });
    let row = cx.debug_bounds("storage-toggle-objects").unwrap();
    cx.simulate_click(row.center(), Default::default());
    manager.read_with(cx, |app, _| {
        assert!(app.storage_ui.open.contains("objects"))
    });
}
