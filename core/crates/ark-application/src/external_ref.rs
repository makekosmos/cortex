use serde::{Deserialize, Serialize};

/// JSON input for the `external_refs.upsert` application request.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalRefUpsert {
    pub connector_id: String,
    pub account_id: String,
    pub external_type: String,
    pub external_id: String,
    pub object_id: String,
    #[serde(default)]
    pub revision: Option<String>,
    #[serde(default)]
    pub hash: Option<String>,
    pub state: String,
}
