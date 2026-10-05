//! Persistent appearance editor. Engine owns policy; controls never infer OS support.
//! The page itself is the shared `imago_gpui` editor — this module only wires
//! Manager's Engine slot transport and optimistic theme application.
use crate::app::{ManagerApp, Slot};
use ::gpui::{prelude::*, *};
use serde_json::{json, Value};

pub fn load(app: &mut ManagerApp) {
    app.call("appearance", "appearance.get", json!({}));
}

pub(super) fn editable(app: &ManagerApp) -> bool {
    app.appearance.ready && matches!(app.slots.get("appearance"), Some(Slot::Ready(_)))
}

/// Optimistically fold the patch into the appearance slot so the shell theme
/// tracks the edit, then send it. The shared editor mirrors the same
/// optimistic value on its side until the Engine reply lands in the slot.
pub(crate) fn patch(app: &mut ManagerApp, params: Value, cx: &mut Context<ManagerApp>) {
    if !editable(app) {
        return;
    }
    // Keep the current page in place. A global action would show a bottom
    // banner and reload the view, which shifts the whole window.
    if let Some(Slot::Ready(value)) = app.slots.get_mut("appearance") {
        if let (Some(settings), Some(patch)) = (
            value.get_mut("settings").and_then(Value::as_object_mut),
            params.as_object(),
        ) {
            for (key, item) in patch {
                settings.insert(key.clone(), item.clone());
            }
        }
        let snapshot = value.clone();
        app.appearance.ingest(&snapshot);
    }
    app.refresh("appearance.set", "appearance.set", params);
    cx.notify();
}

pub fn render(
    app: &mut ManagerApp,
    _window: &mut Window,
    cx: &mut Context<ManagerApp>,
) -> AnyElement {
    let (snapshot, status) = match app.slots.get("appearance") {
        Some(Slot::Ready(value)) => (Some(value.clone()), ""),
        Some(Slot::Failed(_)) => (
            None,
            "Настройки внешнего вида недоступны. Сохранённый стиль не изменён.",
        ),
        _ => (None, "Загрузка настроек внешнего вида…"),
    };
    let enabled = editable(app);
    let failed = matches!(app.slots.get("appearance"), Some(Slot::Failed(_)));
    app.appearance_editor.update(cx, |editor, _| {
        editor.configure(imago_gpui::settings::UiStyle::default(), true, enabled);
        if let Some(value) = snapshot {
            editor.ingest(&value);
        }
        if failed {
            editor.reject();
        }
        editor.status(status);
    });
    app.appearance_editor.clone().into_any_element()
}
