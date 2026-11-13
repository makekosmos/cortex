use super::PolicyIdentity;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OutboundProgress {
    pub policy_generation: i64,
    pub initial_snapshot: InitialSnapshot,
    pub scanned_through: i64,
    pub receipt_through: i64,
    pub applied_through: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum InitialSnapshot {
    Pending { through: i64 },
    Complete { through: i64 },
}

impl InitialSnapshot {
    pub fn is_complete(self) -> bool {
        matches!(self, Self::Complete { .. })
    }

    pub fn through(self) -> i64 {
        match self {
            Self::Pending { through } | Self::Complete { through } => through,
        }
    }
}

impl Default for OutboundProgress {
    fn default() -> Self {
        Self {
            policy_generation: 0,
            initial_snapshot: InitialSnapshot::Complete { through: 0 },
            scanned_through: 0,
            receipt_through: 0,
            applied_through: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum OutboundAck {
    DurableReceipt,
    Applied,
}

/// Selects the progress generation. Identity-guarded writes verify the full
/// accepted peer policy before changing progress.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum OutboundProgressPolicy {
    Generation(i64),
    Identity(PolicyIdentity),
}

/// One durable update to a peer's committed-feed delivery progress.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum OutboundProgressMutation {
    Begin,
    CompleteSnapshot,
    RecordScan { scanned_through: i64 },
    Acknowledge { position: i64, ack: OutboundAck },
}

/// Request to update committed-feed progress for one peer.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OutboundProgressWrite {
    pub peer_device_id: String,
    pub policy: OutboundProgressPolicy,
    pub mutation: OutboundProgressMutation,
}

impl OutboundProgressWrite {
    pub fn new(
        peer_device_id: String,
        policy: OutboundProgressPolicy,
        mutation: OutboundProgressMutation,
    ) -> Self {
        Self {
            peer_device_id,
            policy,
            mutation,
        }
    }

    pub fn weight(&self) -> usize {
        serde_json::to_vec(self).map_or(usize::MAX, |bytes| bytes.len())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OutboundProgressWriteError {
    PolicyChanged,
    PolicyStorage(String),
    ProgressStorage(String),
}

impl std::fmt::Display for OutboundProgressWriteError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PolicyChanged => formatter.write_str("outbound_feed_policy_changed"),
            Self::PolicyStorage(error) | Self::ProgressStorage(error) => formatter.write_str(error),
        }
    }
}

impl std::error::Error for OutboundProgressWriteError {}
