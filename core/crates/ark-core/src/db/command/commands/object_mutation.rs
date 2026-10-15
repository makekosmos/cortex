mod book_cover;
mod canonical_game;
mod delete;
mod upsert;

pub(crate) use book_cover::LocalBookCoverCommand;
pub(crate) use canonical_game::CanonicalGameUpsertCommand;
pub(crate) use delete::LocalObjectDeleteCommand;
pub(crate) use upsert::LocalObjectUpsertCommand;

fn serialized_weight(value: &impl serde::Serialize) -> usize {
    serde_json::to_vec(value)
        .map(|bytes| bytes.len())
        .unwrap_or(usize::MAX)
}
