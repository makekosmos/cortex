#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LinkedAppUsageSummary {
    pub seconds: i64,
    pub session_count: i64,
    pub last_played_at: Option<String>,
}

/// Summarize active usage for app IDs linked to a source object. Legacy app IDs
/// may encode versioned executable paths, so include every tracked row with the
/// same canonical executable identity. Dangling link IDs remain in the query
/// to preserve historical behavior for usage rows that outlive tracked_apps.
pub fn summarize_linked_app_usage(
    conn: &Connection,
    source_object_id: &str,
    link_types: &[&str],
) -> Result<LinkedAppUsageSummary, String> {
    if link_types.is_empty() {
        return Ok(LinkedAppUsageSummary::default());
    }
    let link_type_placeholders = (2..=link_types.len() + 1)
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let link_sql = format!(
        "SELECT target_object_id FROM object_links
         WHERE source_object_id = ?1 AND link_type IN ({link_type_placeholders})"
    );
    let mut link_values = vec![source_object_id.to_string()];
    link_values.extend(link_types.iter().map(|link_type| (*link_type).to_string()));
    let linked_ids = conn
        .prepare(&link_sql)
        .map_err(|error| error.to_string())?
        .query_map(params_from_iter(link_values.iter()), |row| {
            row.get::<_, String>(0)
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    if linked_ids.is_empty() {
        return Ok(LinkedAppUsageSummary::default());
    }

    let mut stmt = conn
        .prepare("SELECT id, normalized_exe_path, process_name FROM tracked_apps")
        .map_err(|error| error.to_string())?;
    let tracked_apps = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    let identity_by_id: HashMap<String, String> = tracked_apps
        .iter()
        .map(|(id, path, process)| (id.clone(), canonical_app_key(path, process)))
        .collect();
    let linked_identities: HashSet<&str> = linked_ids
        .iter()
        .filter_map(|id| identity_by_id.get(id).map(String::as_str))
        .collect();
    let mut app_ids = linked_ids;
    for (id, _, _) in tracked_apps {
        if identity_by_id
            .get(&id)
            .is_some_and(|identity| linked_identities.contains(identity.as_str()))
            && !app_ids.contains(&id)
        {
            app_ids.push(id);
        }
    }

    let placeholders = (1..=app_ids.len())
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT COALESCE(SUM(foreground_ms) / 1000, 0), COUNT(id),
                MAX(COALESCE(ended_at, started_at))
         FROM usage_sessions WHERE tracked_app_id IN ({placeholders})"
    );
    conn.query_row(&sql, params_from_iter(app_ids.iter()), |row| {
        Ok(LinkedAppUsageSummary {
            seconds: row.get(0)?,
            session_count: row.get(1)?,
            last_played_at: row.get(2)?,
        })
    })
    .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linked_app_usage_includes_version_splits_and_dangling_ids() {
        let conn = Connection::open_in_memory().expect("database opens");
        conn.execute_batch(
            "CREATE TABLE object_links (
                source_object_id TEXT NOT NULL, target_object_id TEXT NOT NULL,
                link_type TEXT NOT NULL
             );
             CREATE TABLE tracked_apps (
                id TEXT PRIMARY KEY, normalized_exe_path TEXT NOT NULL,
                process_name TEXT NOT NULL
             );
             CREATE TABLE usage_sessions (
                id TEXT PRIMARY KEY, tracked_app_id TEXT NOT NULL, foreground_ms INTEGER NOT NULL,
                started_at TEXT NOT NULL, ended_at TEXT
             );
             INSERT INTO object_links VALUES
                ('game', 'old-id', 'game'), ('game', 'missing-id', 'game-usage'),
                ('game', 'ignored-id', 'unrelated');
             INSERT INTO tracked_apps VALUES
                ('old-id', 'c:\\games\\demo\\app-1.0.1\\game.exe', 'game.exe'),
                ('new-id', 'c:\\games\\demo\\app-1.0.2\\game.exe', 'game.exe');
             INSERT INTO usage_sessions VALUES
                ('s1', 'old-id', 1800000, '2026-01-01', '2026-01-02'),
                ('s2', 'new-id', 600000, '2026-01-03', NULL),
                ('s3', 'missing-id', 300000, '2026-01-04', NULL);",
        )
        .expect("fixture creates");

        let result = summarize_linked_app_usage(&conn, "game", &["game", "game-usage"])
            .expect("linked usage loads");
        assert_eq!(result.seconds, 2700);
        assert_eq!(result.session_count, 3);
        assert_eq!(result.last_played_at.as_deref(), Some("2026-01-04"));
        assert_eq!(
            summarize_linked_app_usage(&conn, "game", &[]).expect("empty types load"),
            LinkedAppUsageSummary::default()
        );
    }
}
