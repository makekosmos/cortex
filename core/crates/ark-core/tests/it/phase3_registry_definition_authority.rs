#![allow(clippy::unwrap_used)]

use ark_core::canonical_types::definitions::canonical_type_registrations;

#[test]
fn definitions_are_the_registry_hash_authority() {
    // Nine canonical types, task and project second versions, and core.task.
    assert_eq!(canonical_type_registrations().unwrap().len(), 12);
}
