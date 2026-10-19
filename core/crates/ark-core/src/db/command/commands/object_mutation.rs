mod book_cover;
mod canonical_game;

pub(crate) use book_cover::LocalBookCoverCommand;
pub(crate) use canonical_game::CanonicalGameUpsertCommand;

fn serialized_weight(value: &impl serde::Serialize) -> usize {
    serde_json::to_vec(value)
        .map(|bytes| bytes.len())
        .unwrap_or(usize::MAX)
}
