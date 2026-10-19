use ark_application::sync_storage::{
    CommittedFeedDelivery, CommittedFeedDeliveryPage, CommittedFeedEntry, PolicyBoundFeedPage,
};
use rusqlite::{params, Connection, OptionalExtension};

/// Coalesce changes per entity at a pinned watermark and project the current
/// entity payload inside the same SQLite read snapshot. This is a state feed,
/// not an event history: only the latest row per locator is returned.
fn json_data_by_locator(
    conn: &Connection,
    sql: &str,
    entity_id: &str,
) -> Result<Option<serde_json::Map<String, serde_json::Value>>, String> {
    let raw = conn
        .query_row(sql, [entity_id], |row| row.get::<_, String>(0))
        .optional()
        .map_err(|error| error.to_string())?;
    raw.map(|raw| {
        let mut value: serde_json::Value =
            serde_json::from_str(&raw).map_err(|error| error.to_string())?;
        let map = value
            .as_object_mut()
            .ok_or_else(|| "committed_feed_projection_not_object".to_string())?;
        map.remove("id");
        Ok(std::mem::take(map))
    })
    .transpose()
}

fn project_feed_entity(
    conn: &Connection,
    entry: &CommittedFeedEntry,
) -> Result<Option<ark_domain::types::SyncEntity>, String> {
    use serde_json::{Map, Value};

    if entry.entity_type == "object"
        && conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM task_merge_bootstraps WHERE object_id = ?1)",
                [&entry.entity_id],
                |row| row.get::<_, bool>(0),
            )
            .map_err(|error| error.to_string())?
    {
        return Ok(None);
    }

    let data = if entry.deleted {
        conn.query_row(
            "SELECT type_id FROM sync_tombstones WHERE id = ?1 AND entity_type = ?2",
            params![entry.entity_id, entry.entity_type],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()
        .map_err(|error| error.to_string())?
        .flatten()
        .map(|type_id| Map::from_iter([("typeId".to_owned(), Value::String(type_id))]))
        .unwrap_or_default()
    } else {
        let data = match entry.entity_type.as_str() {
            "object" => crate::objects::get_object(conn, &entry.entity_id)?
                .map(|item| serde_json::to_value(item).map_err(|error| error.to_string()))
                .transpose()?
                .and_then(|mut value| {
                    value.as_object_mut().map(|map| {
                        map.remove("id");
                        std::mem::take(map)
                    })
                }),
            "todo" => json_data_by_locator(
                conn,
                "SELECT json_object(
                    'id', id, 'title', title, 'notes', notes, 'priority', priority,
                    'scheduledDate', scheduled_date, 'deadline', deadline,
                    'reminderDate', reminder_date,
                    'isToday', json(CASE WHEN is_today != 0 THEN 'true' ELSE 'false' END),
                    'isEvening', json(CASE WHEN is_evening != 0 THEN 'true' ELSE 'false' END),
                    'isSomeday', json(CASE WHEN is_someday != 0 THEN 'true' ELSE 'false' END),
                    'isCompleted', json(CASE WHEN is_completed != 0 THEN 'true' ELSE 'false' END),
                    'completedAt', completed_at,
                    'isCancelled', json(CASE WHEN is_cancelled != 0 THEN 'true' ELSE 'false' END),
                    'cancelledAt', cancelled_at,
                    'isTrashed', json(CASE WHEN is_trashed != 0 THEN 'true' ELSE 'false' END),
                    'sortOrder', sort_order, 'headingId', heading_id, 'projectId', project_id,
                    'areaId', area_id,
                    'tagIds', CASE WHEN json_valid(tag_ids) THEN json(tag_ids) ELSE json('[]') END,
                    'checklistItems', CASE WHEN json_valid(checklist_items)
                        THEN json(checklist_items) ELSE json('[]') END,
                    'recurrenceRule', CASE WHEN json_valid(recurrence_rule)
                        THEN json(recurrence_rule) ELSE NULL END,
                    'createdAt', created_at
                 ) FROM todos WHERE id = ?1",
                &entry.entity_id,
            )?,
            "project" => json_data_by_locator(
                conn,
                "SELECT json_object(
                    'id', id, 'title', title, 'notes', notes, 'status', status,
                    'scheduledDate', scheduled_date, 'deadline', deadline,
                    'sortOrder', sort_order, 'colorTag', color_tag, 'areaId', area_id,
                    'createdAt', created_at
                 ) FROM projects WHERE id = ?1",
                &entry.entity_id,
            )?,
            "tag" => json_data_by_locator(
                conn,
                "SELECT json_object(
                    'id', id, 'title', title, 'color', color, 'createdAt', created_at
                 )
                 FROM tags WHERE id = ?1",
                &entry.entity_id,
            )?,
            "tracked_app" => json_data_by_locator(
                conn,
                "SELECT json_object(
                    'id', id, 'platform', platform, 'exePath', exe_path,
                    'normalizedExePath', normalized_exe_path, 'processName', process_name,
                    'displayName', display_name, 'publisher', publisher, 'iconRef', icon_ref,
                    'firstSeenAt', first_seen_at, 'lastSeenAt', last_seen_at
                 ) FROM tracked_apps WHERE id = ?1",
                &entry.entity_id,
            )?,
            "object_type" => json_data_by_locator(
                conn,
                "SELECT json_object(
                    'id', id, 'name', name, 'schemaJson', schema_json,
                    'uiSchemaJson', ui_schema_json, 'createdAt', created_at,
                    'updatedAt', updated_at,
                    'systemLocked', json(CASE WHEN system_locked != 0 THEN 'true' ELSE 'false' END)
                 ) FROM object_types WHERE id = ?1",
                &entry.entity_id,
            )?,
            "object_link" => json_data_by_locator(
                conn,
                "SELECT json_object(
                    'id', id, 'sourceObjectId', source_object_id,
                    'targetObjectId', target_object_id, 'linkType', link_type,
                    'createdAt', created_at
                 ) FROM object_links WHERE id = ?1",
                &entry.entity_id,
            )?,
            "usage_session" => crate::usage_history::load_usage_session(conn, &entry.entity_id)?
                .map(|item| serde_json::to_value(item).map_err(|error| error.to_string()))
                .transpose()?
                .and_then(|mut value| {
                    value.as_object_mut().map(|map| {
                        map.remove("id");
                        std::mem::take(map)
                    })
                }),
            "usage_event" => crate::usage_history::load_usage_event(conn, &entry.entity_id)?
                .map(|item| serde_json::to_value(item).map_err(|error| error.to_string()))
                .transpose()?
                .and_then(|mut value| {
                    value.as_object_mut().map(|map| {
                        map.remove("id");
                        std::mem::take(map)
                    })
                }),
            "usage_day" => crate::usage::load_usage_day(conn, &entry.entity_id)?
                .map(|item| serde_json::to_value(item).map_err(|error| error.to_string()))
                .transpose()?
                .and_then(|mut value| {
                    value.as_object_mut().map(|map| {
                        map.remove("id");
                        std::mem::take(map)
                    })
                }),
            _ => {
                return Err(format!(
                    "unsupported_committed_feed_entity_type:{}",
                    entry.entity_type
                ));
            }
        };
        let Some(data) = data else {
            return Ok(None);
        };
        data
    };

    Ok(Some(ark_domain::types::SyncEntity {
        entity_type: entry.entity_type.clone(),
        id: entry.entity_id.clone(),
        data,
        hlc: entry.revision.clone(),
        deleted: entry.deleted.then_some(true),
        origin_device_id: entry.origin_device_id.clone(),
        origin_seq: entry.origin_sequence.map(|sequence| sequence as u64),
    }))
}

pub fn read_committed_feed_delivery_page(
    conn: &Connection,
    after_position: i64,
    high_watermark: i64,
    limit: usize,
    max_delivery_bytes: usize,
) -> Result<CommittedFeedDeliveryPage, String> {
    read_committed_feed_delivery_page_with_hook(
        conn,
        after_position,
        high_watermark,
        limit,
        max_delivery_bytes,
        || {},
    )
}

pub fn read_committed_feed_delivery_page_for_peer(
    conn: &Connection,
    peer_device_id: &str,
    expected_identity: &ark_application::sync_storage::PolicyIdentity,
    after_position: i64,
    high_watermark: i64,
    limit: usize,
    max_delivery_bytes: usize,
) -> Result<PolicyBoundFeedPage, String> {
    conn.execute_batch("SAVEPOINT committed_feed_policy_snapshot")
        .map_err(|error| error.to_string())?;
    let result = (|| -> Result<_, String> {
        let policy = crate::data_platform::load_accepted_peer_sync_policy(conn, peer_device_id)?;
        let identity = policy.as_ref().map_or_else(
            ark_application::sync_policy::default_sync_policy_identity,
            |policy| policy.identity(),
        );
        if &identity != expected_identity {
            return Err("outbound_feed_policy_changed".into());
        }
        let profile = policy
            .as_ref()
            .filter(|policy| policy.active)
            .map(|policy| policy.selective_profile());
        let page = read_committed_feed_delivery_page_with_profile(
            conn,
            after_position,
            high_watermark,
            limit,
            max_delivery_bytes,
            || {},
            profile.as_ref(),
        )?;
        Ok(PolicyBoundFeedPage { identity, page })
    })();
    match result {
        Ok(page) => conn
            .execute_batch("RELEASE committed_feed_policy_snapshot")
            .map(|()| page)
            .map_err(|error| error.to_string()),
        Err(error) => {
            let _ = conn.execute_batch(
                "ROLLBACK TO committed_feed_policy_snapshot;
                 RELEASE committed_feed_policy_snapshot",
            );
            Err(error)
        }
    }
}

fn read_committed_feed_delivery_page_with_hook(
    conn: &Connection,
    after_position: i64,
    high_watermark: i64,
    limit: usize,
    max_delivery_bytes: usize,
    after_selection: impl FnOnce(),
) -> Result<CommittedFeedDeliveryPage, String> {
    read_committed_feed_delivery_page_with_profile(
        conn,
        after_position,
        high_watermark,
        limit,
        max_delivery_bytes,
        after_selection,
        None,
    )
}

fn read_committed_feed_delivery_page_with_profile(
    conn: &Connection,
    after_position: i64,
    high_watermark: i64,
    limit: usize,
    max_delivery_bytes: usize,
    after_selection: impl FnOnce(),
    profile: Option<&ark_domain::selective_sync::SelectiveSyncProfile>,
) -> Result<CommittedFeedDeliveryPage, String> {
    if after_position < 0 || high_watermark < after_position {
        return Err("invalid_committed_feed_cursor".into());
    }
    if limit == 0 || limit > super::MAX_FEED_PAGE_SIZE {
        return Err("committed_feed_page_limit_exceeded".into());
    }
    conn.execute_batch("SAVEPOINT committed_feed_delivery_snapshot")
        .map_err(|error| error.to_string())?;
    let result = (|| -> Result<CommittedFeedDeliveryPage, String> {
        let snapshot = super::committed_feed_snapshot(conn)?;
        if after_position < snapshot.retained_through {
            return Err("feed_gap_resync_required".into());
        }
        if high_watermark > snapshot.high_watermark {
            return Err("committed_feed_watermark_from_future".into());
        }
        let mut statement = conn
            .prepare(
                "SELECT feed.position, feed.entity_type, feed.entity_id, feed.revision,
                    feed.deleted, feed.origin_device_id, feed.origin_sequence
             FROM ark_committed_feed_heads AS head
             JOIN ark_committed_feed AS feed ON feed.position = head.position
             WHERE head.position > ?1 AND head.position <= ?2
             ORDER BY head.position ASC LIMIT ?3",
            )
            .map_err(|error| error.to_string())?;
        let entries = statement
            .query_map(
                params![after_position, high_watermark, limit as i64],
                |row| {
                    Ok(CommittedFeedEntry {
                        position: row.get(0)?,
                        entity_type: row.get(1)?,
                        entity_id: row.get(2)?,
                        revision: row.get(3)?,
                        deleted: row.get::<_, i64>(4)? != 0,
                        origin_device_id: row.get(5)?,
                        origin_sequence: row.get(6)?,
                    })
                },
            )
            .map_err(|error| error.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| error.to_string())?;
        drop(statement);
        after_selection();

        let through_position = entries.last().map(|entry| entry.position);
        let projected = entries
            .iter()
            .filter_map(|entry| project_feed_entity(conn, entry).transpose())
            .collect::<Result<Vec<_>, _>>()?;
        let projected = match profile {
            Some(profile) => {
                ark_domain::selective_sync::filter_entities_for_profile(profile, &projected)
            }
            None => projected,
        };
        let projected = projected
            .into_iter()
            .map(|entity| ((entity.entity_type.clone(), entity.id.clone()), entity))
            .collect::<std::collections::HashMap<_, _>>();

        let mut deliveries = Vec::with_capacity(entries.len());
        let mut serialized_bytes = 0usize;
        for entry in entries {
            let Some(entity) = projected
                .get(&(entry.entity_type.clone(), entry.entity_id.clone()))
                .cloned()
            else {
                continue;
            };
            let entity_bytes = serde_json::to_vec(&entity)
                .map_err(|error| format!("committed_feed_projection_serialize:{error}"))?
                .len();
            if entity_bytes > max_delivery_bytes {
                if deliveries.is_empty() {
                    return Ok(CommittedFeedDeliveryPage::TooLarge {
                        position: entry.position,
                        entity_id: entry.entity_id,
                        serialized_bytes: entity_bytes,
                        max_bytes: max_delivery_bytes,
                    });
                }
                break;
            }
            if serialized_bytes.saturating_add(entity_bytes) > max_delivery_bytes {
                break;
            }
            serialized_bytes += entity_bytes;
            deliveries.push(CommittedFeedDelivery { entry, entity });
        }
        if deliveries.is_empty() {
            if let Some(through_position) = through_position {
                return Ok(CommittedFeedDeliveryPage::SkipTo { through_position });
            }
        }
        Ok(CommittedFeedDeliveryPage::Deliveries(deliveries))
    })();

    match result {
        Ok(page) => conn
            .execute_batch("RELEASE committed_feed_delivery_snapshot")
            .map(|()| page)
            .map_err(|error| error.to_string()),
        Err(error) => {
            let _ = conn.execute_batch(
                "ROLLBACK TO committed_feed_delivery_snapshot;
                 RELEASE committed_feed_delivery_snapshot",
            );
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::read_committed_feed_delivery_page_with_hook;
    use crate::committed_feed::append_committed_change;
    use ark_application::sync_storage::CommittedFeedDeliveryPage;
    use ark_domain::types::ArkObject;

    #[test]
    fn projection_keeps_selected_locator_and_payload_in_one_read_snapshot() {
        let directory = tempfile::tempdir().expect("temporary directory opens");
        let path = directory.path().join("feed-snapshot.db");
        let path = path.to_str().expect("temporary path is UTF-8");
        let reader = crate::open::open_db(path).expect("reader opens");
        reader
            .execute_batch(crate::schema::CREATE_TABLES)
            .expect("schema initializes");
        reader
            .execute(
                "INSERT INTO object_types(
                     id, name, schema_json, ui_schema_json, created_at, updated_at
                 )
                 VALUES ('note_obj', 'Note', '{}', '{}', '2026-10-16T00:00:00.000Z',
                         '2026-10-16T00:00:00.000Z')",
                [],
            )
            .expect("object type persists");
        let writer = crate::open::open_db(path).expect("writer opens");
        writer
            .execute_batch(crate::schema::CREATE_TABLES)
            .expect("writer schema initializes");
        let initial = ArkObject {
            id: "snapshot-object".into(),
            type_id: "note_obj".into(),
            type_version: "0.0.0-legacy".into(),
            title: "Selected payload".into(),
            content_json: serde_json::json!({"type":"doc","content":[]}),
            props_json: serde_json::json!({}),
            created_at: "2026-10-16T00:00:00.000Z".into(),
            updated_at: "2026-10-16T00:00:00.000Z".into(),
            deleted_at: None,
        };
        crate::objects::upsert_object(&reader, &initial).expect("initial object persists");
        append_committed_change(&reader, "object", &initial.id, "r1", false, None, None)
            .expect("initial feed entry persists");

        let page = read_committed_feed_delivery_page_with_hook(&reader, 0, 1, 8, 1_000_000, || {
            let mut updated = initial.clone();
            updated.title = "Newer concurrent payload".into();
            updated.updated_at = "2026-10-16T00:00:01.000Z".into();
            crate::objects::upsert_object(&writer, &updated)
                .expect("concurrent object update persists");
            append_committed_change(&writer, "object", &updated.id, "r2", false, None, None)
                .expect("concurrent feed entry persists");
        })
        .expect("delivery page reads");

        let deliveries = match page {
            CommittedFeedDeliveryPage::Deliveries(deliveries) => deliveries,
            CommittedFeedDeliveryPage::SkipTo { .. } => panic!("feed page only skipped entries"),
            CommittedFeedDeliveryPage::TooLarge { .. } => panic!("small test payload is bounded"),
        };
        assert_eq!(deliveries.len(), 1);
        assert_eq!(deliveries[0].entry.position, 1);
        assert_eq!(deliveries[0].entry.revision, "r1");
        assert_eq!(
            deliveries[0].entity.data.get("title"),
            Some(&serde_json::json!("Selected payload"))
        );
    }
}
