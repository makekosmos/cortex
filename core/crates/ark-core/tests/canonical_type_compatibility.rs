use ark_core::canonical_types::definitions::{
    canonical_type_registrations, current_canonical_version,
};
use ark_core::canonical_types::validation::{validate_canonical, CanonicalValidationCode};
use serde_json::json;

#[test]
fn canonical_definitions_and_validation_remain_available_from_ark_core() {
    let registrations = canonical_type_registrations().expect("canonical definitions are valid");
    let registration = registrations
        .iter()
        .find(|registration| {
            registration.type_id == "core.task" && registration.version == "0.2.0"
        })
        .expect("core.task is registered");

    assert_eq!(
        current_canonical_version("core.task").expect("canonical versions are valid"),
        Some("0.2.0".to_owned())
    );
    assert_eq!(
        validate_canonical(
            registration,
            &json!({
                "state": {"kind": "open", "at": null},
                "planning": {"kind": "inbox", "period": "any", "date": null},
                "deadline": null,
                "reminders": [],
                "recurrence": null,
                "priority": "none",
                "checklist": [],
                "extensions": {}
            }),
            &json!({"type": "doc", "content": [{"type": "paragraph"}]}),
        ),
        Ok(())
    );
    assert_eq!(
        validate_canonical(registration, &json!({"is_today": true}), &json!({})),
        Err(
            ark_core::canonical_types::validation::CanonicalValidationError {
                code: CanonicalValidationCode::InvalidField,
                pointer: "/state".to_owned(),
                keyword: Some("required".to_owned()),
            }
        )
    );
}
