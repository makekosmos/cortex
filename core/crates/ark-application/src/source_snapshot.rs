pub const GENERIC_SOURCE_TYPE_IDS: [&str; 9] = [
    "note_obj",
    "task_obj",
    "project_obj",
    "tag_obj",
    "person_obj",
    "image_obj",
    "time_entry_obj",
    "game_obj",
    "book_obj",
];

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SourceSnapshot {
    pub objects: Vec<GenericObjectRow>,
    pub todos: Vec<TodoRow>,
    pub projects: Vec<ProjectRow>,
    pub tags: Vec<TagRow>,
    /// `None` means the legacy table is absent; `Some(vec![])` means it exists and is empty.
    pub areas: Option<Vec<AreaRow>>,
    /// `None` means the legacy table is absent; `Some(vec![])` means it exists and is empty.
    pub headings: Option<Vec<HeadingRow>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenericObjectRow {
    pub id: String,
    pub type_id: String,
    pub type_version: String,
    pub title: String,
    pub content_json: String,
    pub props_json: String,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TodoRow {
    pub id: String,
    pub title: String,
    pub notes: Option<String>,
    pub priority: i64,
    pub scheduled_date: Option<String>,
    pub deadline: Option<String>,
    pub reminder_date: Option<String>,
    pub is_today: i64,
    pub is_evening: i64,
    pub is_someday: i64,
    pub is_completed: i64,
    pub completed_at: Option<String>,
    pub is_cancelled: i64,
    pub cancelled_at: Option<String>,
    pub is_trashed: i64,
    pub sort_order: i64,
    pub heading_id: Option<String>,
    pub project_id: Option<String>,
    pub area_id: Option<String>,
    pub tag_ids: String,
    pub checklist_items: String,
    pub recurrence_rule: Option<String>,
    pub created_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectRow {
    pub id: String,
    pub title: String,
    pub notes: Option<String>,
    pub status: String,
    pub scheduled_date: Option<String>,
    pub deadline: Option<String>,
    pub sort_order: i64,
    pub color_tag: Option<String>,
    pub area_id: Option<String>,
    pub created_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TagRow {
    pub id: String,
    pub title: String,
    pub color: Option<String>,
    pub created_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AreaRow {
    pub id: String,
    pub title: String,
    pub sort_order: i64,
    pub created_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeadingRow {
    pub id: String,
    pub title: String,
    pub sort_order: i64,
    pub project_id: String,
}
