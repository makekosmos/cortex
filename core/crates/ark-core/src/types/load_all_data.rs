use serde::Serialize;

use super::{
    Area, ArkObject, Heading, ObjectLink, ObjectType, Project, Tag, TodoItem, TrackedApp,
    UsageEvent, UsageSession,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadAllData {
    pub todos: Vec<TodoItem>,
    pub projects: Vec<Project>,
    pub areas: Vec<Area>,
    pub tags: Vec<Tag>,
    pub headings: Vec<Heading>,
    pub tracked_apps: Vec<TrackedApp>,
    pub usage_sessions: Vec<UsageSession>,
    pub usage_events: Vec<UsageEvent>,
    pub objects: Vec<ArkObject>,
    pub object_types: Vec<ObjectType>,
    pub object_type_summaries: Vec<crate::type_registry::TypeSummary>,
    pub object_type_versions: Vec<crate::type_registry::TypeVersion>,
    pub object_type_aliases: Vec<crate::type_registry::AliasRecord>,
    pub object_links: Vec<ObjectLink>,
}
