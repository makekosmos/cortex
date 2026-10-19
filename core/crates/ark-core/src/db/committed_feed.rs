use ark_application::sync_storage::{OutboundAck, OutboundProgress};
use rusqlite::Connection;

pub const MAX_FEED_DELIVERY_BYTES: usize = ark_sync::protocol::MAX_BATCH_BYTES - 512;
pub const MAX_INBOX_PAYLOAD_BYTES: usize = ark_sync::protocol::MAX_BATCH_BYTES;
pub use ark_store::committed_feed::InboxState;

include!("committed_feed/progress.rs");
include!("committed_feed/inbox.rs");
include!("committed_feed/tests.rs");

#[cfg(test)]
mod policy_tests;
