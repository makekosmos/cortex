use crate::service::ArkService;
use serde_json::{json, Value};

fn core_types() -> Vec<(&'static str, Value)> {
    vec![
        (
            "core.task",
            json!({
                "state": {"kind":"open", "at":null},
                "planning": {"kind":"inbox", "period":"any", "date":null},
                "deadline": null,
                "reminders": [],
                "recurrence": null,
                "priority": "none",
                "checklist": [],
                "extensions": {}
            }),
        ),
        (
            "core.note",
            json!({"description":null, "extensions":{}}),
        ),
        (
            "core.project",
            json!({"status":"active", "scheduledAt":null, "dueAt":null, "color":null, "extensions":{}}),
        ),
        (
            "core.tag",
            json!({"color":null, "extensions":{}}),
        ),
        (
            "core.person",
            json!({"firstName":null, "lastName":null, "patronymic":null, "birthDate":null, "extensions":{}}),
        ),
        (
            "core.image",
            json!({"fileName":null, "mimeType":null, "sizeBytes":null, "width":null, "height":null, "resolution":null, "altText":"", "extensions":{}}),
        ),
        (
            "core.time-entry",
            json!({"startedAt":"2026-10-24T12:00:00.000Z", "endedAt":null, "billable":false, "source":"manual", "taskTitle":null, "extensions":{}}),
        ),
        (
            "core.game",
            json!({"playStatus":null, "userRating":null, "genres":[], "platforms":[], "released":null, "description":null, "extensions":{}}),
        ),
        (
            "core.book",
            json!({"author":null, "isbn":null, "pageCount":null, "language":null, "publisher":null, "publishedDate":null, "sourceUrl":null, "extensions":{}}),
        ),
    ]
}

#[tokio::test]
async fn every_core_type_round_trips_through_upsert_get_and_list() {
    let dir = crate::test_support::temp_dir();
    let db_path = dir.path().join("ark.db");
    let service = ArkService::open(db_path.to_string_lossy().into_owned())
        .await
        .expect("service opens with current core registrations");

    for (index, (type_id, props)) in core_types().into_iter().enumerate() {
        let id = format!("core-type-{index}");
        service
            .call(
                "upsert_object",
                json!({
                    "object": {
                        "id": id.clone(),
                        "typeId": type_id,
                        "typeVersion": "0.1.0",
                        "title": type_id,
                        "contentJson": {"type":"doc", "content":[{"type":"paragraph"}]},
                        "propsJson": props,
                        "createdAt": "2026-10-24T12:00:00.000Z",
                        "updatedAt": "2026-10-24T12:00:00.000Z",
                        "deletedAt": null
                    }
                }),
            )
            .await
            .expect("core type upsert succeeds");

        let stored = service
            .call("get_object", json!({"id": id}))
            .await
            .expect("core type get succeeds");
        assert_eq!(stored["typeId"], type_id);
        assert_eq!(stored["typeVersion"], "0.1.0");
        assert_eq!(stored["propsJson"], props);

        let listed = service
            .call("list_objects_by_type", json!({"typeId": type_id}))
            .await
            .expect("core type list succeeds");
        assert_eq!(listed.as_array().map(Vec::len), Some(1));
        assert_eq!(listed[0]["id"], id);
        assert_eq!(listed[0]["typeId"], type_id);
    }
}
