use ark_domain::types::{ArkObject, ObjectLink};
use serde::Serialize;
use std::fmt;

pub const BOOK_TYPE_ID: &str = "com.kosmos.book";
pub const IMAGE_TYPE_ID: &str = "com.kosmos.image";
pub const BOOK_COVER_LINK_TYPE: &str = "cover-image";
pub const CANONICAL_VERSION: &str = "1.0.0";

#[derive(Debug, Clone, Serialize)]
pub struct LocalBookCoverWrite {
    pub book_id: String,
    pub source_ref: Option<String>,
    pub existing_image_id: Option<String>,
    pub alt_text: String,
    pub device_id: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LocalBookCoverFacts {
    pub book: Option<ArkObject>,
    pub image: Option<ArkObject>,
    pub existing_cover_links: Vec<ObjectLink>,
    pub link_created_at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LocalBookCoverPlan {
    pub book_id: String,
    pub device_id: String,
    pub deleted_link_ids: Vec<String>,
    pub new_link: Option<ObjectLink>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LocalBookCoverDecision {
    Unchanged,
    Change(LocalBookCoverPlan),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookCoverError {
    pub code: &'static str,
    pub pointer: Option<String>,
}

impl BookCoverError {
    fn new(code: &'static str) -> Self {
        Self {
            code,
            pointer: None,
        }
    }
}

impl fmt::Display for BookCoverError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "canonical_facade:{}", self.code)?;
        if let Some(pointer) = &self.pointer {
            write!(formatter, ":{pointer}")?;
        }
        Ok(())
    }
}

impl std::error::Error for BookCoverError {}

pub fn plan_local_book_cover(
    write: LocalBookCoverWrite,
    facts: LocalBookCoverFacts,
) -> Result<LocalBookCoverDecision, BookCoverError> {
    if write.source_ref.is_some() && write.existing_image_id.is_some() {
        return Err(BookCoverError::new("conflicting_source"));
    }
    if write.source_ref.is_some() {
        return Err(BookCoverError::new("source_requires_existing_image"));
    }
    let book = facts
        .book
        .ok_or_else(|| BookCoverError::new("missing_book"))?;
    validate_object(&book, BOOK_TYPE_ID, "wrong_book_type")?;

    let image_id = match (&write.existing_image_id, facts.image) {
        (Some(_), None) => return Err(BookCoverError::new("missing_image")),
        (Some(id), Some(image)) => {
            if image.type_id != IMAGE_TYPE_ID {
                return Err(BookCoverError::new("wrong_image_type"));
            }
            if image.deleted_at.is_some() {
                return Err(BookCoverError::new("deleted_image"));
            }
            validate_object(&image, IMAGE_TYPE_ID, "wrong_image_type")?;
            Some(id.clone())
        }
        (None, _) => None,
    };
    let desired_link_id = match &image_id {
        Some(image_id) => Some(canonical_book_cover_link_id(&write.book_id, image_id)),
        None => None,
    };
    let already_current = desired_link_id.as_ref().is_some_and(|desired| {
        facts
            .existing_cover_links
            .iter()
            .any(|link| &link.id == desired)
    }) && facts.existing_cover_links.len() == 1;
    if already_current {
        return Ok(LocalBookCoverDecision::Unchanged);
    }

    let new_link = desired_link_id
        .zip(image_id)
        .map(|(id, image_id)| ObjectLink {
            id,
            source_object_id: write.book_id.clone(),
            target_object_id: image_id,
            link_type: BOOK_COVER_LINK_TYPE.into(),
            created_at: facts.link_created_at,
        });
    Ok(LocalBookCoverDecision::Change(LocalBookCoverPlan {
        book_id: write.book_id,
        device_id: write.device_id,
        deleted_link_ids: facts
            .existing_cover_links
            .into_iter()
            .map(|link| link.id)
            .collect(),
        new_link,
    }))
}

fn validate_object(
    object: &ArkObject,
    expected_type: &str,
    wrong_type: &'static str,
) -> Result<(), BookCoverError> {
    if object.type_id != expected_type {
        return Err(BookCoverError::new(wrong_type));
    }
    if object.type_version != CANONICAL_VERSION {
        return Err(BookCoverError::new("wrong_version"));
    }
    let registration = ark_domain::canonical_type::definitions::canonical_type_registrations()
        .map_err(|_| BookCoverError::new("definition_invariant"))?
        .into_iter()
        .find(|registration| {
            registration.type_id == expected_type && registration.version == CANONICAL_VERSION
        })
        .ok_or_else(|| BookCoverError::new("wrong_type"))?;
    ark_domain::canonical_type::validation::validate_canonical(
        &registration,
        &object.props_json,
        &object.content_json,
    )
    .map_err(|error| BookCoverError {
        code: "malformed_canonical",
        pointer: Some(error.pointer),
    })
}

pub fn canonical_book_cover_link_id(book_id: &str, image_id: &str) -> String {
    use sha2::{Digest, Sha256};

    let mut hash = Sha256::new();
    hash.update(format!(
        "kosmos-link-v1\0{book_id}\0{BOOK_COVER_LINK_TYPE}\0{image_id}"
    ));
    format!("lnk*{:x}", hash.finalize())
}

#[cfg(test)]
#[path = "book_cover_tests.rs"]
mod tests;
