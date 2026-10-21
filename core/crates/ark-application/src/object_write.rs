use ark_domain::object_write::ObjectWriteSnapshot;
use ark_domain::types::{ArkObject, ObjectLink};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjectWriteOrigin {
    Local {
        device_id: Option<String>,
        expected_snapshot: Option<ObjectWriteSnapshot>,
    },
    AuthenticatedRemote {
        peer_id: String,
    },
    TrustedRemote,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ReceiptOutcome {
    Applied,
    Stale,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StoredObjectReceipt {
    pub change_seq: i64,
    pub payload_sha256: String,
    pub payload_json: String,
    pub outcome: ReceiptOutcome,
    pub resulting_hlc: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewObjectReceipt {
    pub entity_id: String,
    pub representation_kind: String,
    pub source_hlc: String,
    pub source_origin_device_id: Option<String>,
    pub type_id: String,
    pub payload_sha256: String,
    pub payload_json: String,
    pub outcome: ReceiptOutcome,
    pub resulting_hlc: Option<String>,
    pub authenticated_peer_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectWriteResult {
    pub receipt: Option<StoredObjectReceipt>,
    pub duplicate: bool,
}

pub trait ObjectWriteSession {
    fn resolve_type_identity(
        &self,
        type_id: &str,
        requested_version: Option<&str>,
    ) -> Result<(String, String), String>;
    fn object_snapshot(&self, id: &str) -> Result<ObjectWriteSnapshot, String>;
    fn current_revision(&self, id: &str) -> Result<Option<String>, String>;
    fn is_peer_removed(&self, peer_id: &str) -> Result<bool, String>;
    fn persist_object(&self, object: &ArkObject, links: &[ObjectLink]) -> Result<(), String>;
    fn record_local_revision(&self, id: &str, device_id: Option<&str>) -> Result<String, String>;
    fn record_remote_revision(&self, id: &str, source_hlc: &str) -> Result<(), String>;
    fn load_receipt(
        &self,
        entity_id: &str,
        representation_kind: &str,
        source_hlc: &str,
    ) -> Result<Option<StoredObjectReceipt>, String>;
    fn insert_receipt(&self, receipt: NewObjectReceipt) -> Result<StoredObjectReceipt, String>;
}
