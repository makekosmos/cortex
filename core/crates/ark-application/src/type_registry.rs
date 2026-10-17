use ark_domain::canonical_type::{TypeSummary, TypeVersion};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeGetResponse {
    pub summary: TypeSummary,
    pub definition: TypeVersion,
}
