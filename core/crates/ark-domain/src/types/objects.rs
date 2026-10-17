use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

// Persistent object records and write commands shared by storage and the API.

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArkObject {
    pub id: String,
    pub type_id: String,
    pub type_version: String,
    pub title: String,
    #[serde(default = "default_content_json")]
    pub content_json: Value,
    #[serde(default = "default_props_json")]
    pub props_json: Value,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArkObjectSummary {
    pub id: String,
    pub type_id: String,
    pub type_version: String,
    pub title: String,
    #[serde(default = "default_props_json")]
    pub props_json: Value,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArkObjectWrite {
    pub id: String,
    pub type_id: String,
    #[serde(default)]
    pub type_version: Option<String>,
    pub title: String,
    #[serde(default = "default_content_json")]
    pub content_json: Value,
    #[serde(default = "default_props_json")]
    pub props_json: Value,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

impl ArkObjectWrite {
    pub fn with_type_version(self, type_version: String) -> ArkObject {
        ArkObject {
            id: self.id,
            type_id: self.type_id,
            type_version,
            title: self.title,
            content_json: self.content_json,
            props_json: self.props_json,
            created_at: self.created_at,
            updated_at: self.updated_at,
            deleted_at: self.deleted_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectType {
    pub id: String,
    pub name: String,
    pub schema_json: String,
    pub ui_schema_json: String,
    pub created_at: String,
    pub updated_at: String,
    pub system_locked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectLink {
    pub id: String,
    pub source_object_id: String,
    pub target_object_id: String,
    pub link_type: String,
    pub created_at: String,
}

fn default_content_json() -> Value {
    json!({
        "type": "doc",
        "content": [{ "type": "paragraph" }]
    })
}

fn default_props_json() -> Value {
    json!({})
}
