use ark_application::book_cover::{
    plan_local_book_cover, BookCoverError, LocalBookCoverDecision, LocalBookCoverFacts,
    LocalBookCoverWrite, BOOK_COVER_LINK_TYPE,
};
use ark_domain::hlc::HLC;
use rusqlite::{Connection, Transaction, TransactionBehavior};
use std::fmt;

#[derive(Debug)]
pub enum BookCoverOperationError {
    Storage(String),
    Invalid(BookCoverError),
}

impl fmt::Display for BookCoverOperationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(error) => formatter.write_str(error),
            Self::Invalid(error) => fmt::Display::fmt(error, formatter),
        }
    }
}

impl From<String> for BookCoverOperationError {
    fn from(error: String) -> Self {
        Self::Storage(error)
    }
}

pub fn apply_local_book_cover(
    connection: &mut Connection,
    write: LocalBookCoverWrite,
) -> Result<bool, crate::writer::TransactionFailure<BookCoverOperationError>> {
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(crate::writer::TransactionFailure::Begin)?;
    let changed = apply_local_book_cover_in(&transaction, write)
        .map_err(crate::writer::TransactionFailure::Operation)?;
    transaction
        .commit()
        .map_err(crate::writer::TransactionFailure::Commit)?;
    Ok(changed)
}

fn apply_local_book_cover_in(
    transaction: &Transaction<'_>,
    write: LocalBookCoverWrite,
) -> Result<bool, BookCoverOperationError> {
    let book = crate::objects::get_object(transaction, &write.book_id)?;
    let image = write
        .existing_image_id
        .as_deref()
        .map(|id| crate::objects::get_object(transaction, id))
        .transpose()?
        .flatten();
    let existing_cover_links: Vec<_> =
        crate::objects::list_object_links_for_source(transaction, &write.book_id)?
            .into_iter()
            .filter(|link| {
                link.source_object_id == write.book_id && link.link_type == BOOK_COVER_LINK_TYPE
            })
            .collect();
    let link_created_at = HLC::now(&write.device_id).wall_time;
    let decision = plan_local_book_cover(
        write,
        LocalBookCoverFacts {
            book,
            image,
            existing_cover_links,
            link_created_at,
        },
    )
    .map_err(BookCoverOperationError::Invalid)?;
    let LocalBookCoverDecision::Change(plan) = decision else {
        return Ok(false);
    };

    for id in &plan.deleted_link_ids {
        crate::objects::delete_object_link(transaction, id)?;
    }
    if let Some(link) = &plan.new_link {
        crate::objects::upsert_object_link(transaction, link)?;
    }

    let book_hlc = crate::sync_versions::bump_sync_version_vector(
        transaction,
        "object",
        &plan.book_id,
        &plan.device_id,
        false,
    )?;
    crate::sync_tombstones::delete(transaction, &plan.book_id)?;
    crate::sync_versions::write_object_sync_version(transaction, &plan.book_id, &book_hlc, false)?;
    if let Some(link) = &plan.new_link {
        let hlc = crate::sync_versions::bump_sync_version_vector(
            transaction,
            "object_link",
            &link.id,
            &plan.device_id,
            false,
        )?;
        crate::sync_tombstones::delete(transaction, &link.id)?;
        crate::sync_versions::write_object_sync_version(transaction, &link.id, &hlc, false)?;
    }
    for id in &plan.deleted_link_ids {
        let hlc = crate::sync_versions::bump_sync_version_vector(
            transaction,
            "object_link",
            id,
            &plan.device_id,
            true,
        )?;
        crate::sync_tombstones::record(transaction, "object_link", id, None, &hlc)?;
    }
    if let Some(image_id) = plan
        .new_link
        .as_ref()
        .map(|link| link.target_object_id.as_str())
    {
        crate::sync_versions::bump_sync_version_vector(
            transaction,
            "object",
            image_id,
            &plan.device_id,
            false,
        )?;
        crate::sync_tombstones::delete(transaction, image_id)?;
    }
    Ok(true)
}
