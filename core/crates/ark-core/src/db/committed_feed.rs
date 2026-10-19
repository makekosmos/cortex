use ark_application::sync_storage::{
    CommittedFeedDelivery, CommittedFeedDeliveryPage, CommittedFeedEntry, FeedSnapshot,
    OutboundAck, OutboundProgress,
};
use rusqlite::{params, Connection, OptionalExtension};

pub const MAX_FEED_PAGE_SIZE: usize = 128;
pub const MAX_FEED_DELIVERY_BYTES: usize = ark_sync::protocol::MAX_BATCH_BYTES - 512;
pub const MAX_INBOX_PAYLOAD_BYTES: usize = ark_sync::protocol::MAX_BATCH_BYTES;
pub use ark_store::committed_feed::InboxState;

include!("committed_feed/progress.rs");
include!("committed_feed/log.rs");
include!("committed_feed/delivery.rs");
include!("committed_feed/inbox.rs");
include!("committed_feed/tests.rs");

#[cfg(test)]
mod policy_tests;
