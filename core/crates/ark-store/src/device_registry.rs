use ark_domain::device_registry::{
    state::DeviceRegistry, DeviceId, DeviceRecord, DisplayName, RegistryEvent, RegistryHead,
    SigningPublicKey,
};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use sha2::Digest;

pub const MAX_REGISTRY_EVENTS_PAGE_SIZE: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceRegistryEntry {
    pub device_id: String,
    pub display_name: Option<String>,
    pub enabled: bool,
}

pub fn load_entries(connection: &Connection) -> Result<Vec<DeviceRegistryEntry>, String> {
    let mut statement = connection
        .prepare(
            "SELECT device_id, display_name, disabled_event_id IS NULL
             FROM device_registry_projection ORDER BY device_id",
        )
        .map_err(|error| error.to_string())?;
    let entries = statement
        .query_map([], |row| {
            Ok(DeviceRegistryEntry {
                device_id: row.get(0)?,
                display_name: row.get(1)?,
                enabled: row.get(2)?,
            })
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    Ok(entries)
}

pub fn is_device_known(connection: &Connection, device_id: &str) -> Result<bool, String> {
    connection
        .query_row(
            "SELECT 1 FROM device_registry_projection WHERE device_id = ?1",
            [device_id],
            |_| Ok(()),
        )
        .optional()
        .map(|known| known.is_some())
        .map_err(|error| error.to_string())
}

pub fn is_device_admitted(
    connection: &Connection,
    device_id: &str,
) -> Result<Option<bool>, String> {
    connection
        .query_row(
            "SELECT disabled_event_id IS NULL FROM device_registry_projection
             WHERE device_id = ?1",
            [device_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())
}

pub fn event_json_for_actor_sequence(
    connection: &Connection,
    actor_device_id: &str,
    actor_sequence: u64,
) -> Result<Option<String>, String> {
    connection
        .query_row(
            "SELECT event_json FROM device_registry_events
             WHERE actor_device_id = ?1 AND actor_sequence = ?2",
            params![actor_device_id, actor_sequence],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| error.to_string())
}

pub fn persist_event_and_projection(
    transaction: &Transaction<'_>,
    event: &RegistryEvent,
    event_hash: &str,
    event_json: &str,
    device: &DeviceRecord,
) -> Result<(), String> {
    let registry_position: i64 = transaction
        .query_row(
            "SELECT COALESCE(MAX(registry_position), 0) + 1 FROM device_registry_events",
            [],
            |row| row.get(0),
        )
        .map_err(|error| error.to_string())?;
    let actor_sequence =
        i64::try_from(event.actor_sequence()).map_err(|error| error.to_string())?;
    transaction
        .execute(
            "INSERT INTO device_registry_events (
                registry_position, actor_device_id, actor_sequence, event_id,
                target_device_id, previous_event_hash, event_hash, event_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                registry_position,
                event.actor_device_id().as_str(),
                actor_sequence,
                event.event_id(),
                event.target_device_id().as_str(),
                event.previous_event_hash(),
                event_hash,
                event_json,
            ],
        )
        .map_err(|error| error.to_string())?;
    persist_projection(transaction, device)
}

pub fn registry_digest(connection: &Connection) -> Result<String, String> {
    let mut statement = connection
        .prepare(
            "SELECT actor_device_id, actor_sequence, event_hash
             FROM device_registry_events ORDER BY actor_device_id, actor_sequence",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, u64>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    let bytes = serde_json::to_vec(&rows).map_err(|error| error.to_string())?;
    let digest = sha2::Sha256::digest(bytes);
    Ok(digest.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn persist_projection(transaction: &Transaction<'_>, device: &DeviceRecord) -> Result<(), String> {
    transaction
        .execute(
            "INSERT INTO device_registry_projection (
                device_id, signing_public_key, key_epoch, admission_epoch, display_name,
                disabled_event_id, display_name_hlc, updated_hlc, updated_event_id
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(device_id) DO UPDATE SET
                signing_public_key = excluded.signing_public_key,
                key_epoch = excluded.key_epoch,
                admission_epoch = excluded.admission_epoch,
                display_name = excluded.display_name,
                disabled_event_id = excluded.disabled_event_id,
                display_name_hlc = excluded.display_name_hlc,
                updated_hlc = excluded.updated_hlc,
                updated_event_id = excluded.updated_event_id",
            params![
                device.device_id.as_str(),
                device.signing_public_key.as_bytes().as_slice(),
                device.key_epoch,
                device.admission_epoch,
                device.display_name.as_ref().map(DisplayName::as_str),
                device.disabled_event_id,
                device.display_name_hlc,
                device.updated_hlc,
                device.updated_event_id,
            ],
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub fn load_registry(connection: &Connection) -> Result<DeviceRegistry, String> {
    let mut registry = DeviceRegistry::default();
    let mut statement = connection
        .prepare(
            "SELECT device_id, signing_public_key, key_epoch, admission_epoch,
                    display_name, disabled_event_id, display_name_hlc, updated_hlc,
                    updated_event_id
             FROM device_registry_projection ORDER BY device_id",
        )
        .map_err(|error| error.to_string())?;
    let devices = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Vec<u8>>(1)?,
                row.get::<_, u64>(2)?,
                row.get::<_, u64>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, Option<String>>(8)?,
            ))
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    drop(statement);
    for (id, key, key_epoch, admission_epoch, name, disabled, name_hlc, updated_hlc, event_id) in
        devices
    {
        let key: [u8; 32] = key
            .try_into()
            .map_err(|_| "invalid_device_registry_key_length".to_string())?;
        registry.add_trust_anchor(DeviceRecord {
            device_id: DeviceId::parse(id).map_err(|error| error.to_string())?,
            signing_public_key: SigningPublicKey::from_bytes(key)
                .map_err(|error| error.to_string())?,
            key_epoch,
            admission_epoch,
            display_name: name
                .map(DisplayName::parse)
                .transpose()
                .map_err(|error| error.to_string())?,
            disabled_event_id: disabled,
            display_name_hlc: name_hlc,
            updated_hlc,
            updated_event_id: event_id,
        });
    }

    let mut statement = connection
        .prepare(
            "SELECT actor_device_id, actor_sequence, event_hash
             FROM device_registry_events AS event
             WHERE actor_sequence = (
                 SELECT MAX(actor_sequence) FROM device_registry_events
                 WHERE actor_device_id = event.actor_device_id
             ) ORDER BY actor_device_id",
        )
        .map_err(|error| error.to_string())?;
    let heads = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, u64>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    for (actor, sequence, event_hash) in heads {
        registry.set_head(RegistryHead {
            actor_device_id: DeviceId::parse(actor).map_err(|error| error.to_string())?,
            actor_sequence: sequence,
            event_hash,
        });
    }
    Ok(registry)
}

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
