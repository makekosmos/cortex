use super::*;
use crate::canonical_types::validation::validate_canonical;
use crate::type_registry::TypeRegistration;
use serde_json::{json, Value};

fn core_task() -> Result<TypeRegistration, String> {
    canonical_type_registrations()?
        .into_iter()
        .find(|r| r.type_id == "core.task" && r.version == "0.1.0")
        .ok_or_else(|| "core.task 0.1.0 is not registered".to_owned())
}

fn empty_doc() -> Value {
    json!({"type": "doc", "content": [{"type": "paragraph"}]})
}

fn default_props() -> Value {
    json!({
        "state": {"kind": "open", "at": null},
        "planning": {"kind": "inbox", "period": "any", "date": null},
        "deadline": null,
        "reminders": [],
        "recurrence": null,
        "priority": "none",
        "checklist": [],
        "extensions": {}
    })
}

#[test]
fn core_types_are_the_only_registered_first_party_versions() -> Result<(), String> {
    let registrations = canonical_type_registrations()?;
    let type_ids: Vec<_> = registrations
        .iter()
        .map(|registration| registration.type_id.as_str())
        .collect();
    assert_eq!(
        type_ids,
        vec![
            "core.task",
            "core.note",
            "core.project",
            "core.tag",
            "core.person",
            "core.image",
            "core.time-entry",
            "core.game",
            "core.book",
        ]
    );
    for registration in registrations {
        assert_eq!(registration.version, "0.1.0", "{}", registration.type_id);
        assert!(registration.aliases.is_empty(), "{}", registration.type_id);
    }
    Ok(())
}

#[test]
fn core_task_is_the_current_version_and_claims_no_legacy_alias() -> Result<(), String> {
    let registration = core_task()?;
    assert!(registration.aliases.is_empty());
    assert_eq!(
        current_canonical_version("core.task")?,
        Some("0.1.0".to_owned())
    );
    Ok(())
}

#[test]
fn core_task_accepts_the_default_shape() -> Result<(), String> {
    let registration = core_task()?;
    validate_canonical(&registration, &default_props(), &empty_doc())
        .map_err(|error| format!("{error:?}"))
}

#[test]
fn core_task_rejects_an_unknown_planning_kind() -> Result<(), String> {
    let registration = core_task()?;
    let mut props = default_props();
    props["planning"]["kind"] = json!("calendar");
    assert!(validate_canonical(&registration, &props, &empty_doc()).is_err());
    Ok(())
}

#[test]
fn core_task_rejects_a_field_outside_the_contract() -> Result<(), String> {
    let registration = core_task()?;
    let mut props = default_props();
    props["is_today"] = json!(true);
    assert!(validate_canonical(&registration, &props, &empty_doc()).is_err());
    Ok(())
}
