use super::{canonical_schema_hash, AliasRecord, TypeRegistration};

mod book;
mod core_task;
mod game;
mod image;
mod note;
mod person;
mod project;
mod registrations;
mod tag;
mod time_entry;

pub(crate) use book::BOOK_DEFINITION;
pub(crate) use core_task::CORE_TASK_DEFINITION;
pub(crate) use game::GAME_DEFINITION;
pub(crate) use image::IMAGE_DEFINITION;
pub(crate) use note::NOTE_DEFINITION;
pub(crate) use person::PERSON_DEFINITION;
pub(crate) use project::PROJECT_DEFINITION;
pub use registrations::{
    canonical_type_registrations, current_canonical_version, is_canonical_version,
};

pub fn is_supported_core_task_version(version: &str) -> bool {
    matches!(version, "0.1.0" | "0.2.0")
}

pub(crate) use tag::TAG_DEFINITION;
pub(crate) use time_entry::TIME_ENTRY_DEFINITION;
