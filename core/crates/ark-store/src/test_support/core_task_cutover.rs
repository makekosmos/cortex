use std::path::Path;

use serde_json::json;

/// Seeds the legacy task row used to verify service-start cutover behavior.
pub struct LegacyTaskCutoverFixture;

impl LegacyTaskCutoverFixture {
    pub fn seed(path: &Path) -> Result<(), String> {
        let path = path
            .to_str()
            .ok_or_else(|| "legacy task database path is not UTF-8".to_string())?;
        let connection = crate::open::open_db(path)?;
        crate::open::init_schema(&connection)?;
        let content = json!({
            "content": [{
                "content": [{"text": "", "type": "text"}],
                "type": "paragraph"
            }],
            "type": "doc"
        });
        let props = json!({
            "canceledAt": null,
            "checklist": [],
            "completedAt": null,
            "dueAt": null,
            "priority": "none",
            "recurrence": null,
            "reminderAt": null,
            "scheduledAt": null,
            "status": "todo",
            "extensions": {
                "area_id": null,
                "billable": false,
                "cancelled_at": null,
                "checklist_items": [],
                "completed_at": null,
                "created_at": "2026-10-04T22:01:08.744412+00:00",
                "deadline": null,
                "description": null,
                "fuel_cost": null,
                "heading_id": null,
                "is_cancelled": false,
                "is_completed": false,
                "is_evening": false,
                "is_someday": false,
                "is_today": true,
                "is_trashed": false,
                "price": null,
                "priority": 0,
                "project_id": null,
                "recurrence_rule": null,
                "reminder_date": null,
                "scheduled_date": null,
                "significance": null,
                "sort_order": 0,
                "status": "todo",
                "system_kind": null,
                "tag_ids": []
            }
        });
        connection
            .execute(
                "INSERT INTO objects (
                    id, type_id, type_version, title, content_json, props_json,
                    created_at, updated_at
                 ) VALUES (?1, 'com.kosmos.task', '1.1.0', ?1, ?2, ?3, ?4, ?4)",
                rusqlite::params![
                    "legacy-task",
                    content.to_string(),
                    props.to_string(),
                    "2026-10-04T22:01:08.744412+00:00"
                ],
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
}
