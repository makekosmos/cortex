use crate::{app::Slot, views::View};
use gpui::{px, size, TestAppContext};
use serde_json::json;

#[gpui::test]
fn storage_starts_collapsed_and_keeps_every_size_on_one_left_edge(cx: &mut TestAppContext) {
    let (manager, cx) = crate::a11y_tests::launch(cx);
    cx.simulate_resize(size(px(1000.), px(1600.)));
    manager.update(cx, |app, cx| {
        app.view = View::Data;
        app.slots.insert(
            "data.storage".into(),
            Slot::Ready(json!({
                "total_bytes": 4096,
                "categories": [
                    {"id":"database", "label":"База данных", "bytes":3072,
                     "detail":[{"label":"Журнал", "bytes":1024}]},
                    {"id":"legacy_quarantine", "label":"Устаревшие данные", "bytes":1024}
                ]
            })),
        );
        cx.notify();
    });
    cx.update(|_, cx| cx.refresh_windows());
    assert!(cx.debug_bounds("data-storage-list").is_none());
    let header = cx.debug_bounds("data-storage-toggle").unwrap();
    cx.simulate_click(header.center(), Default::default());
    cx.update(|_, cx| cx.refresh_windows());
    assert!(cx.debug_bounds("data-storage-list").is_some());
    let total = cx.debug_bounds("storage-size-Хранилище на диске").unwrap();
    for label in [
        "storage-size-База данных",
        "storage-size-Журнал",
        "storage-size-Устаревшие данные",
    ] {
        let value = cx.debug_bounds(label).unwrap();
        assert!(
            (f32::from(total.left() - value.left())).abs() <= 1.,
            "unaligned size for {label}"
        );
    }
    let tree = cx
        .update(|window, _| window.debug_a11y_tree_json())
        .unwrap();
    assert!(tree.contains("Очистить"));
    cx.simulate_click(header.center(), Default::default());
    cx.update(|_, cx| cx.refresh_windows());
    assert!(cx.debug_bounds("data-storage-list").is_none());
}
