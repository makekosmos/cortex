use ark_domain::canonical_type::{AliasRecord, TypeSummary, TypeVersion};
use ark_domain::types::{
    ArkObject, ObjectLink, ObjectType, TrackedApp, UsageEvent, UsageSession,
};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadAllData {
    pub tracked_apps: Vec<TrackedApp>,
    pub usage_sessions: Vec<UsageSession>,
    pub usage_events: Vec<UsageEvent>,
    pub objects: Vec<ArkObject>,
    pub object_types: Vec<ObjectType>,
    pub object_type_summaries: Vec<TypeSummary>,
    pub object_type_versions: Vec<TypeVersion>,
    pub object_type_aliases: Vec<AliasRecord>,
    pub object_links: Vec<ObjectLink>,
}
