use ark_domain::device_registry::RegistryEvent;
use rusqlite::{params, Connection};

pub const MAX_REGISTRY_EVENTS_PAGE_SIZE: usize = 64;

pub fn load_events_page(
    connection: &Connection,
    after_position: i64,
    high_watermark: Option<i64>,
    limit: usize,
) -> Result<(i64, usize, Vec<RegistryEvent>), String> {
    connection
        .execute_batch("SAVEPOINT device_registry_events_page")
        .map_err(|error| error.to_string())?;
    let result = (|| -> Result<_, String> {
        if after_position < 0 || limit == 0 || limit > MAX_REGISTRY_EVENTS_PAGE_SIZE {
            return Err("device_registry_page_bounds".into());
        }
        let high_watermark = match high_watermark {
            Some(high_watermark) if high_watermark >= after_position => high_watermark,
            Some(_) => return Err("device_registry_page_cursor".into()),
            None => connection
                .query_row(
                    "SELECT COALESCE(MAX(registry_position), 0) FROM device_registry_events",
                    [],
                    |row| row.get(0),
                )
                .map_err(|error| error.to_string())?,
        };
        let event_count = connection
            .query_row(
                "SELECT COUNT(*) FROM device_registry_events WHERE registry_position <= ?1",
                [high_watermark],
                |row| row.get::<_, i64>(0),
            )
            .map_err(|error| error.to_string())?;
        let event_count = usize::try_from(event_count)
            .map_err(|_| "negative_device_registry_event_count".to_string())?;
        let mut statement = connection
            .prepare(
                "SELECT event_json FROM device_registry_events
                 WHERE registry_position > ?1 AND registry_position <= ?2
                 ORDER BY registry_position LIMIT ?3",
            )
            .map_err(|error| error.to_string())?;
        let events = statement
            .query_map(
                params![after_position, high_watermark, limit as i64],
                |row| row.get::<_, String>(0),
            )
            .map_err(|error| error.to_string())?
            .map(|raw| {
                let raw = raw.map_err(|error| error.to_string())?;
                serde_json::from_str(&raw)
                    .map_err(|error| format!("invalid registry event: {error}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok((high_watermark, event_count, events))
    })();
    match result {
        Ok(page) => connection
            .execute_batch("RELEASE device_registry_events_page")
            .map(|()| page)
            .map_err(|error| error.to_string()),
        Err(error) => {
            let _ = connection.execute_batch(
                "ROLLBACK TO device_registry_events_page;
                 RELEASE device_registry_events_page",
            );
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{load_events_page, MAX_REGISTRY_EVENTS_PAGE_SIZE};
    use rusqlite::Connection;

    #[test]
    fn empty_registry_page_has_a_stable_zero_watermark() {
        let connection = Connection::open_in_memory().expect("database opens");
        connection
            .execute_batch(crate::schema::CREATE_TABLES)
            .expect("schema initializes");

        let page = load_events_page(&connection, 0, None, MAX_REGISTRY_EVENTS_PAGE_SIZE)
            .expect("empty page reads");

        assert_eq!(page, (0, 0, Vec::new()));
    }
}
