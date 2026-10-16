use crate::application::sync_storage::{
    CommittedFeedDelivery, CommittedFeedDeliveryPage, CommittedFeedEntry, FeedSnapshot,
    InitialSnapshot, OutboundAck, OutboundProgress,
};
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};

pub const MAX_FEED_PAGE_SIZE: usize = 128;
pub const MAX_FEED_DELIVERY_BYTES: usize = crate::protocol::MAX_BATCH_BYTES - 512;
pub const MAX_INBOX_PAYLOAD_BYTES: usize = crate::protocol::MAX_BATCH_BYTES;
const MAX_PENDING_INBOX_ROWS: i64 = 10_000;
const MAX_PENDING_INBOX_BYTES: i64 = 32 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InboxState {
    Received,
    Pending,
    Applied,
    Rejected,
}

include!("committed_feed/progress.rs");
include!("committed_feed/log.rs");
include!("committed_feed/delivery.rs");
include!("committed_feed/inbox.rs");
include!("committed_feed/tests.rs");
