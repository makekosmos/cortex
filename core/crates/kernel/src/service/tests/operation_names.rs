//! Every operation the service accepts must read `section.verb`.

use super::*;
use crate::service::definitions::OPERATION_NAMES;

#[test]
fn operation_names_are_lowercase_dotted_and_complete() {
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for name in OPERATION_NAMES {
        assert!(
            name.split('.').all(|segment| !segment.is_empty()
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
                && segment.as_bytes()[0].is_ascii_lowercase()),
            "{name} is not lowercase dot-separated words"
        );
        assert!(
            name.contains('.'),
            "{name} is not section.verb — it has no dot"
        );
        assert!(seen.insert((*name).to_string()), "{name} is listed twice");
    }

    // The table is the whole surface: the deserializer names every variant it
    // knows when it rejects an unknown one, so an operation added without a
    // line here fails this equality instead of quietly escaping the rule.
    let error = serde_json::from_value::<Request>(json!({"operation": "naming.probe"}))
        .expect_err("an unknown operation must be rejected")
        .to_string();
    let known = error
        .split("expected one of ")
        .nth(1)
        .expect("the rejection lists known operations")
        .split(", ")
        .map(|token| token.trim_matches('`').to_string())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(known, seen, "the operation table drifted from the router");
}

#[test]
fn old_underscore_operations_are_gone() {
    for old in [
        "list_objects",
        "get_object",
        "upsert_object",
        "delete_object",
        "list_object_links",
        "db_backup",
        "start_sync",
        "get_sync_kv",
        "types_list",
        "device_registry",
    ] {
        assert!(
            serde_json::from_value::<Request>(json!({"operation": old})).is_err(),
            "{old} still decodes"
        );
    }
}
