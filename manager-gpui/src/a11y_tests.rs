//! Accessibility-tree tests (KOS-142).
//!
//! The vendored gpui `TestWindow` activates accessibility immediately
//! (vendor/gpui-pre-0.3.6-patched/PATCH.md), so every drawn frame's accesskit
//! nodes are readable through `Window::debug_a11y_tree_json`. These tests
//! render the real Manager shell on the test platform and assert that every
//! interactive node carries a Russian accessible name, plus a checked-in
//! snapshot of the whole tree.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use gpui::{AppContext, Entity, TestAppContext, VisualTestContext};
use serde_json::{json, Value};

use crate::app::ManagerApp;

/// Roles an assistive user can activate or edit — the union that must never
/// appear in the tree without a name.
const INTERACTIVE_ROLES: &[&str] = &[
    "Button",
    "CheckBox",
    "ComboBox",
    "Link",
    "ListBoxOption",
    "MenuItem",
    "MenuItemCheckBox",
    "MenuItemRadio",
    "RadioButton",
    "ScrollBar",
    "SearchInput",
    "Slider",
    "SpinButton",
    "Switch",
    "Tab",
    "TextInput",
    "MultilineTextInput",
    "PasswordInput",
    "TreeItem",
];

/// Builds the same view tree as `main.rs` — ManagerApp inside
/// `gpui_component::Root` — on the deterministic test platform. The Engine
/// worker just fails to connect in the background; slots stay empty.
fn launch(cx: &mut TestAppContext) -> (Entity<ManagerApp>, &mut VisualTestContext) {
    cx.update(gpui_component::init);
    cx.update(imago_gpui::theme::apply);
    let slot: Rc<RefCell<Option<Entity<ManagerApp>>>> = Rc::new(RefCell::new(None));
    let slot2 = slot.clone();
    let (_root, cx) = cx.add_window_view(move |window, cx| {
        let manager = cx.new(|cx| ManagerApp::new(window, cx));
        *slot2.borrow_mut() = Some(manager.clone());
        gpui_component::Root::new(manager, window, cx)
    });
    let manager = slot.borrow_mut().take().expect("window builder ran");
    (manager, cx)
}

/// Draw a pending frame (product handlers don't always `notify`, so force a
/// redraw), then read the accesskit tree captured at end of frame.
fn a11y_tree(cx: &mut VisualTestContext) -> Value {
    cx.update(|_, cx| cx.refresh_windows());
    cx.run_until_parked();
    let json = cx
        .update(|window, _| window.debug_a11y_tree_json())
        .expect("vendored TestWindow activates a11y; a frame must capture a tree");
    serde_json::from_str(&json).expect("a11y tree is valid JSON")
}

/// `role` / `label` / state flags for each node, keyed by node id.
fn nodes(tree: &Value) -> Vec<(String, Value)> {
    tree["nodes"]
        .as_object()
        .expect("nodes object")
        .values()
        .map(|n| {
            (
                n["aria"]["role"].as_str().unwrap_or_default().to_string(),
                n.clone(),
            )
        })
        .collect()
}

/// Human-readable dump used for the checked-in snapshot. Node ids are
/// ephemeral per frame, so the snapshot keeps role/name/state and the
/// parent-child nesting (depth) instead.
fn snapshot_lines(tree: &Value) -> Vec<String> {
    let nodes = tree["nodes"].as_object().expect("nodes object");
    let children_of = |id: &str| -> Vec<String> {
        nodes[id]["children"]
            .as_array()
            .map(|c| {
                c.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    };
    let mut lines = Vec::new();
    let mut stack: Vec<(String, usize)> =
        vec![(tree["root"].as_str().expect("root id").to_string(), 0)];
    while let Some((id, depth)) = stack.pop() {
        let Some(node) = nodes.get(&id) else { continue };
        let aria = &node["aria"];
        let role = aria["role"].as_str().unwrap_or("?");
        let label = aria["label"].as_str().unwrap_or("");
        let mut extra = String::new();
        if let Some(sel) = aria["selected"].as_bool() {
            extra += &format!(" selected={sel}");
        }
        if let Some(t) = aria["toggled"].as_str() {
            extra += &format!(" toggled={t}");
        }
        if let Some(v) = aria["numeric_value"].as_f64() {
            extra += &format!(" value={v}");
        }
        lines.push(format!("{}{role} \"{label}\"{extra}", "  ".repeat(depth)));
        for child in children_of(&id).into_iter().rev() {
            stack.push((child, depth + 1));
        }
    }
    lines
}

#[gpui::test]
async fn a11y_tree_has_no_unnamed_interactive_nodes(cx: &mut TestAppContext) {
    let (_app, cx) = launch(cx);
    let tree = a11y_tree(cx);

    let nodes = nodes(&tree);
    assert!(
        nodes.len() > 10,
        "expected a populated a11y tree, got {} nodes",
        nodes.len()
    );

    let unnamed: Vec<String> = nodes
        .iter()
        .filter(|(role, n)| {
            INTERACTIVE_ROLES.contains(&role.as_str())
                && n["aria"]["label"]
                    .as_str()
                    .map(|l| l.trim().is_empty())
                    .unwrap_or(true)
        })
        .map(|(role, n)| format!("{role} node {}", n["accesskit_id"]))
        .collect();
    assert!(unnamed.is_empty(), "unnamed interactive nodes: {unnamed:?}");
}

#[gpui::test]
async fn a11y_tree_shell_exposes_russian_names(cx: &mut TestAppContext) {
    let (_app, cx) = launch(cx);
    let tree = a11y_tree(cx);
    let nodes = nodes(&tree);

    let labels: BTreeMap<String, String> = nodes
        .iter()
        .filter_map(|(role, n)| {
            n["aria"]["label"]
                .as_str()
                .map(|l| (l.to_string(), role.clone()))
        })
        .collect();

    // Sidebar navigation, titlebar refresh and window chrome — stable
    // Russian names suitable as locators.
    for expected in [
        "Данные",
        "Затреканное время",
        "Маркетплейс",
        "Настройки",
        "Обновить",
        "Боковая панель",
        "Свернуть",
        "Закрыть",
    ] {
        assert!(
            labels.contains_key(expected),
            "expected a11y name '{expected}' in the tree; got labels: {:?}",
            labels.keys().collect::<Vec<_>>()
        );
    }
    assert_eq!(labels["Данные"], "Button");
    assert_eq!(labels["Обновить"], "Button");
    assert_eq!(labels["Свернуть"], "Button");
    assert_eq!(labels["Боковая панель"], "Switch");
}

/// KOS-287 regression: the usage page renders rows through
/// `v_virtual_list`, so a 600-row report must materialize only the visible
/// window of ListItem nodes — before virtualization every row hit the tree
/// and scrolling stuttered.
#[gpui::test]
async fn usage_list_is_virtualized(cx: &mut TestAppContext) {
    let (manager, cx) = launch(cx);
    let rows: Vec<Value> = (0..600)
        .map(|i| {
            json!({
                "displayName": format!("UT-App-{i:03}"),
                "processName": format!("ut-app-{i}.exe"),
                "normalizedPath": format!("c:\\apps\\ut-app-{i}.exe"),
                "foregroundMs": 600 - i,
                "sessions": 1,
            })
        })
        .collect();
    manager.update(cx, |app, cx| {
        app.view = crate::views::View::Usage;
        app.slots.insert(
            "usage.report".into(),
            crate::app::Slot::Ready(json!({ "topApps": rows })),
        );
        app.rebuild_usage_rows();
        cx.notify();
    });
    let tree = a11y_tree(cx);

    let rendered = nodes(&tree)
        .iter()
        .filter(|(role, n)| {
            role == "ListItem"
                && n["aria"]["label"]
                    .as_str()
                    .is_some_and(|l| l.starts_with("UT-App-"))
        })
        .count();
    assert!(rendered > 0, "usage rows must render");
    assert!(
        rendered < 100,
        "virtual list must not materialize all 600 rows, got {rendered}"
    );
}

/// KOS-287 regression: prepared rows are built once per data/sort/filter
/// change — re-rendering the view must not re-sort or re-format the table
/// (that per-frame O(n log n) was the scroll stutter).
#[gpui::test]
async fn usage_rows_rebuild_only_on_input_change(cx: &mut TestAppContext) {
    let (manager, cx) = launch(cx);
    manager.update(cx, |app, _cx| {
        app.view = crate::views::View::Usage;
        app.slots.insert(
            "usage.report".into(),
            crate::app::Slot::Ready(json!({ "topApps": [
                {
                    "displayName": "Alpha",
                    "processName": "a.exe",
                    "foregroundMs": 10,
                    "sessions": 1,
                },
                {
                    "displayName": "Beta",
                    "processName": "b.exe",
                    "foregroundMs": 20,
                    "sessions": 1,
                },
            ] })),
        );
        app.rebuild_usage_rows();
    });
    // Two renders back-to-back — same inputs, no rebuild.
    let _ = a11y_tree(cx);
    let _ = a11y_tree(cx);
    manager.read_with(cx, |app, _| assert_eq!(app.usage_builds, 1));
    // Sort and filter changes are the only rebuild triggers.
    manager.update(cx, |app, _| {
        app.toggle_usage_sort(crate::views::usage::UsageColumn::Name)
    });
    manager.read_with(cx, |app, _| {
        assert_eq!(app.usage_builds, 2);
        assert_eq!(app.usage_rows[0].name.as_ref(), "Alpha");
    });
    manager.update(cx, |app, _| app.set_usage_show_system(true));
    manager.read_with(cx, |app, _| assert_eq!(app.usage_builds, 3));
}

#[gpui::test]
async fn a11y_tree_matches_snapshot(cx: &mut TestAppContext) {
    let (_app, cx) = launch(cx);
    let tree = a11y_tree(cx);
    let actual = snapshot_lines(&tree).join("\n") + "\n";

    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/snapshots/a11y-shell.txt");
    if std::env::var("A11Y_BLESS").is_ok() {
        std::fs::create_dir_all(std::path::Path::new(path).parent().unwrap())
            .expect("create snapshots dir");
        std::fs::write(path, &actual).expect("write snapshot");
        return;
    }
    let expected = std::fs::read_to_string(path)
        .unwrap_or_else(|_| panic!("missing snapshot {path}; run with A11Y_BLESS=1 to create it"));
    if actual != expected {
        let expected_lines: Vec<&str> = expected.lines().collect();
        let actual_lines: Vec<&str> = actual.lines().collect();
        let mut diff = String::new();
        for i in 0..expected_lines.len().max(actual_lines.len()) {
            let e = expected_lines.get(i).copied().unwrap_or("<missing>");
            let a = actual_lines.get(i).copied().unwrap_or("<missing>");
            if e != a {
                diff.push_str(&format!(
                    "line {}:\n  expected: {e}\n  actual:   {a}\n",
                    i + 1
                ));
            }
        }
        panic!(
            "a11y tree differs from snapshot {path}:\n{diff}\n(run with A11Y_BLESS=1 to accept)"
        );
    }
}

/// KOS-299: an installed `kind:"app"` .kspkg row in «Приложения» renders a
/// named «Открыть» button — the same affordance native app rows expose.
#[gpui::test]
async fn app_package_row_exposes_open_button(cx: &mut TestAppContext) {
    let (manager, cx) = launch(cx);
    manager.update(cx, |app, _cx| {
        app.view = crate::views::View::Packages;
        app.store_tab = crate::views::StoreTab::Installed;
        app.slots.insert(
            "store.apps".into(),
            crate::app::Slot::Ready(json!({ "apps": [] })),
        );
        app.slots.insert(
            "store.installed".into(),
            crate::app::Slot::Ready(json!({ "packages": [
                {
                    "id": "com.kosmos.arcadia",
                    "name": "Arcadia",
                    "version": "1.2.3",
                    "kind": "app",
                    "enabled": true,
                    "catalog_sequence": 7,
                },
                {
                    "id": "com.kosmos.sleepy",
                    "name": "Sleepy",
                    "version": "0.9.9",
                    "kind": "app",
                    "enabled": false,
                    "catalog_sequence": 7,
                },
                {
                    "id": "com.kosmos.bridge",
                    "name": "Some Bridge",
                    "version": "0.1.0",
                    "kind": "bridge",
                    "enabled": true,
                    "catalog_sequence": 7,
                },
            ]})),
        );
    });
    let tree = a11y_tree(cx);
    let nodes = nodes(&tree);
    let open_labels: Vec<String> = nodes
        .iter()
        .filter(|(role, _)| role == "Button")
        .filter_map(|(_, n)| n["aria"]["label"].as_str().map(str::to_string))
        .collect();
    assert!(
        open_labels.contains(&"Открыть Arcadia".to_string()),
        "the app-kind package row must expose «Открыть Arcadia»; got {open_labels:?}"
    );
    assert!(
        !open_labels.contains(&"Открыть Some Bridge".to_string()),
        "a bridge-kind package row must not expose «Открыть»; got {open_labels:?}"
    );

    // A disabled package still renders «Открыть» as a disabled control.
    // The vendored test window does not surface the flag in the a11y tree,
    // so the flag itself — the predicate the row passes to `.disabled(...)` —
    // is what this test asserts.
    assert!(
        open_labels.contains(&"Открыть Sleepy".to_string()),
        "a disabled app row must still render its «Открыть»; got {open_labels:?}"
    );
    assert!(
        crate::views::store::package_open_disabled(&json!({ "enabled": false })),
        "a disabled package must disable «Открыть»"
    );
    assert!(
        !crate::views::store::package_open_disabled(&json!({ "enabled": true })),
        "an enabled package must leave «Открыть» actionable"
    );
}

/// KOS-299: «Открыть» dispatches `packages.open` into the `pkg.open` slot,
/// and the typed Engine error surfaces as the Russian banner text.
#[gpui::test]
async fn package_open_dispatches_and_surfaces_errors(cx: &mut TestAppContext) {
    let (manager, cx) = launch(cx);
    manager.update(cx, |app, _cx| {
        app.open_package("com.kosmos.arcadia".into(), "1.2.3".into());
    });
    manager.read_with(cx, |app, _| {
        assert!(matches!(
            app.slots.get("pkg.open"),
            Some(crate::app::Slot::Loading)
        ));
    });
    manager.update(cx, |app, _cx| {
        app.open_reply(Err("Приложение отключено. Включите его в списке.".into()));
    });
    manager.read_with(cx, |app, _| {
        assert_eq!(
            app.error.as_deref(),
            Some("Приложение отключено. Включите его в списке.")
        );
    });
    // A lease payload without a launch URL is malformed — say so instead of
    // opening nothing.
    manager.update(cx, |app, _cx| {
        app.open_reply(Ok(json!({})));
    });
    manager.read_with(cx, |app, _| {
        assert_eq!(
            app.error.as_deref(),
            Some("Engine не вернул адрес приложения.")
        );
    });
}
