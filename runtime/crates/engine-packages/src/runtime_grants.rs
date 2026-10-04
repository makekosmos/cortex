//! Engine-owned Phase 5 launch authority and closed typed request boundary.
use serde::{de::Error as DeError, Deserialize, Deserializer};
use std::collections::BTreeSet;
use thiserror::Error;

mod compile;
mod grant;
mod operations;
mod requests;
mod store;

#[cfg(test)]
mod tests;

pub use compile::*;
pub use grant::*;
pub use operations::*;
pub use requests::*;
pub use store::*;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GrantError {
    #[error("contract violation: {0}")]
    Contract(&'static str),
    #[error("grant capacity")]
    Capacity,
    #[error("grant expired")]
    Expired,
    #[error("stale generation")]
    StaleGeneration,
    #[error("grant missing")]
    Missing,
    #[error("grant revoked")]
    Revoked,
}

const MAX_RULES: usize = 64;
const MAX_BATCH: usize = 100;
const MAX_FIELDS: usize = 256;
const MAX_RELATIONS: usize = 128;

fn id(s: &str, max: usize) -> Result<String, GrantError> {
    if s.is_empty() || s.len() > max || !s.is_char_boundary(s.len()) {
        return Err(GrantError::Contract("identifier"));
    }
    Ok(s.to_owned())
}
fn unique(xs: &[String]) -> bool {
    let mut seen = BTreeSet::new();
    xs.iter().all(|x| seen.insert(x))
}
fn bounded_list(xs: &[String], max: usize) -> Result<(), GrantError> {
    if xs.len() > max || !unique(xs) {
        return Err(GrantError::Contract("bounded unique list"));
    }
    xs.iter().try_for_each(|x| id(x, 128).map(|_| ()))
}

fn deserialize_unique_strings<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let values = Vec::<String>::deserialize(deserializer)?;
    if unique(&values) {
        Ok(values)
    } else {
        Err(D::Error::custom("duplicate string"))
    }
}
