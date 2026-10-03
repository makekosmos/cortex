//! Static field labels are always rendered. Only the value depends on Engine.
//! Do not wrap a whole settings/about card in slot_or when its schema is known.
use crate::app::{ManagerApp, Slot};
use crate::widgets::kv;
use gpui::Div;
use serde_json::Value;

pub fn field_text(slot: Option<&Slot>, read: impl FnOnce(&Value) -> String) -> String {
    match slot {
        Some(Slot::Ready(value)) => {
            let text = read(value);
            if text.trim().is_empty() {
                "—".into()
            } else {
                text
            }
        }
        Some(Slot::Failed(_)) => "Недоступно".into(),
        Some(Slot::Loading) | None => "Загрузка…".into(),
    }
}

pub fn field_row(
    app: &ManagerApp,
    slot: &str,
    label: &str,
    read: impl FnOnce(&Value) -> String,
) -> Div {
    kv(label, field_text(app.slots.get(slot), read))
}

#[cfg(test)]
mod tests {
    use super::{field_text, Slot};
    use serde_json::json;

    #[test]
    fn pending_and_failed_fields_do_not_read_fake_default_values() {
        assert_eq!(field_text(None, |_| panic!("not ready")), "Загрузка…");
        assert_eq!(
            field_text(Some(&Slot::Loading), |_| panic!("not ready")),
            "Загрузка…"
        );
        assert_eq!(
            field_text(Some(&Slot::Failed("offline".into())), |_| panic!(
                "not ready"
            )),
            "Недоступно"
        );
    }

    #[test]
    fn ready_and_missing_metadata_have_distinct_values() {
        let ready = Slot::Ready(json!({"version":"1.2.3"}));
        assert_eq!(
            field_text(Some(&ready), |v| v["version"].as_str().unwrap().into()),
            "1.2.3"
        );
        assert_eq!(field_text(Some(&ready), |_| String::new()), "—");
    }
}
