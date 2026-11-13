use rusqlite::{params, Connection, OptionalExtension, Transaction};

const IDENTITY_KEYS: [&str; 3] = [
    "iroh.secret_key",
    crate::sync_kv::DEVICE_REGISTRY_LOCAL_DEVICE_ID_KEY,
    "sync.peer_transport_keys",
];

type RegistryEventRow = (
    i64,
    String,
    i64,
    String,
    String,
    Option<String>,
    String,
    String,
);

type RegistryProjectionRow = (
    String,
    Vec<u8>,
    i64,
    i64,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);

struct LiveDeviceIdentity {
    events: Vec<RegistryEventRow>,
    projection: Vec<RegistryProjectionRow>,
    sync_keys: Vec<(&'static str, Option<String>)>,
}

pub(super) fn preserve_live_device_identity(
    live_connection: &Connection,
    staged_target: &std::path::Path,
) -> Result<(), String> {
    let identity = load_live_identity(live_connection)?;

    let mut target = Connection::open(staged_target)
        .map_err(|error| format!("restore staging open failed: {error}"))?;
    let transaction = target
        .transaction()
        .map_err(|error| format!("restore identity transaction failed: {error}"))?;
    replace_registry(&transaction, &identity)?;
    replace_identity_keys(&transaction, &identity.sync_keys)?;
    transaction
        .commit()
        .map_err(|error| format!("restore identity commit failed: {error}"))?;
    verify_live_identity(&target, &identity)?;
    crate::open::check_integrity(&target)
}

fn load_live_identity(connection: &Connection) -> Result<LiveDeviceIdentity, String> {
    let events = connection
        .prepare(
            "SELECT registry_position, actor_device_id, actor_sequence, event_id,
                    target_device_id, previous_event_hash, event_hash, event_json
             FROM device_registry_events ORDER BY registry_position",
        )
        .map_err(|error| error.to_string())?
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
            ))
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    let projection = connection
        .prepare(
            "SELECT device_id, signing_public_key, key_epoch, admission_epoch,
                    display_name, disabled_event_id, display_name_hlc, updated_hlc,
                    updated_event_id
             FROM device_registry_projection ORDER BY device_id",
        )
        .map_err(|error| error.to_string())?
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
            ))
        })
        .map_err(|error| error.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    let sync_keys = IDENTITY_KEYS
        .into_iter()
        .map(|key| {
            let value = connection
                .query_row("SELECT value FROM sync_kv WHERE key = ?1", [key], |row| {
                    row.get(0)
                })
                .optional()
                .map_err(|error| error.to_string())?;
            Ok((key, value))
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(LiveDeviceIdentity {
        events,
        projection,
        sync_keys,
    })
}

fn replace_registry(
    transaction: &Transaction<'_>,
    identity: &LiveDeviceIdentity,
) -> Result<(), String> {
    transaction
        .execute("DELETE FROM device_registry_events", [])
        .map_err(|error| error.to_string())?;
    transaction
        .execute("DELETE FROM device_registry_projection", [])
        .map_err(|error| error.to_string())?;
    for (position, actor, sequence, event_id, target, previous_hash, hash, event_json) in
        &identity.events
    {
        transaction
            .execute(
                "INSERT INTO device_registry_events(
                    registry_position, actor_device_id, actor_sequence, event_id,
                    target_device_id, previous_event_hash, event_hash, event_json
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    position,
                    actor,
                    sequence,
                    event_id,
                    target,
                    previous_hash,
                    hash,
                    event_json
                ],
            )
            .map_err(|error| error.to_string())?;
    }
    for (
        device_id,
        signing_key,
        key_epoch,
        admission_epoch,
        display_name,
        disabled_event_id,
        display_name_hlc,
        updated_hlc,
        updated_event_id,
    ) in &identity.projection
    {
        transaction
            .execute(
                "INSERT INTO device_registry_projection(
                    device_id, signing_public_key, key_epoch, admission_epoch,
                    display_name, disabled_event_id, display_name_hlc, updated_hlc,
                    updated_event_id
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    device_id,
                    signing_key,
                    key_epoch,
                    admission_epoch,
                    display_name,
                    disabled_event_id,
                    display_name_hlc,
                    updated_hlc,
                    updated_event_id
                ],
            )
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn replace_identity_keys(
    transaction: &Transaction<'_>,
    keys: &[(&str, Option<String>)],
) -> Result<(), String> {
    for (key, value) in keys {
        transaction
            .execute("DELETE FROM sync_kv WHERE key = ?1", [key])
            .map_err(|error| error.to_string())?;
        if let Some(value) = value {
            transaction
                .execute(
                    "INSERT INTO sync_kv(key, value) VALUES (?1, ?2)",
                    params![key, value],
                )
                .map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

fn verify_live_identity(
    connection: &Connection,
    expected: &LiveDeviceIdentity,
) -> Result<(), String> {
    let actual = load_live_identity(connection)?;
    if actual.events != expected.events {
        return Err("restored registry event history differs from live history".into());
    }
    if actual.projection != expected.projection {
        return Err("restored registry projection differs from live projection".into());
    }
    if actual.sync_keys != expected.sync_keys {
        return Err("restored device identity keys differ from live identity".into());
    }
    Ok(())
}
