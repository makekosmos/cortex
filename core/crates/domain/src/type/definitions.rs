use super::{canonical_schema_hash, AliasRecord, TypeRegistration};

// First-party type definitions are JSON files, not Rust: a schema change is a
// data change, and the frozen hash in each file (checked by
// `registration_from_literal`) is what makes an accidental edit visible.
mod registrations;

pub(crate) const BOOK_DEFINITION: &str = include_str!("definitions/book.json");
pub(crate) const CORE_TASK_DEFINITION: &str = include_str!("definitions/core_task.json");
pub(crate) const GAME_DEFINITION: &str = include_str!("definitions/game.json");
pub(crate) const IMAGE_DEFINITION: &str = include_str!("definitions/image.json");
pub(crate) const NOTE_DEFINITION: &str = include_str!("definitions/note.json");
pub(crate) const PERSON_DEFINITION: &str = include_str!("definitions/person.json");
pub(crate) const PROJECT_DEFINITION: &str = include_str!("definitions/project.json");
pub(crate) const TAG_DEFINITION: &str = include_str!("definitions/tag.json");
pub(crate) const TIME_ENTRY_DEFINITION: &str = include_str!("definitions/time_entry.json");

pub use registrations::{
    canonical_type_registrations, current_canonical_version, is_canonical_version,
};

pub fn is_supported_core_task_version(version: &str) -> bool {
    matches!(version, "0.1.0" | "0.2.0")
}
