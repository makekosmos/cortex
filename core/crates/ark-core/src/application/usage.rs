use crate::types::{UsageDay, UsageEvent, UsageSession, UsageSpanWrite};
use serde_json::Value;

pub trait UsageWriteSession {
    fn ensure_sequence_migrated(&self, device_id: &str) -> Result<(), String>;
    fn upsert_session(&self, session: &UsageSession) -> Result<(), String>;
    fn delete_session(&self, id: &str) -> Result<(), String>;
    fn upsert_event(&self, event: &UsageEvent) -> Result<(), String>;
    fn delete_event(&self, id: &str) -> Result<(), String>;
    fn upsert_span(&self, span: &UsageSpanWrite) -> Result<Vec<UsageDay>, String>;
    fn record_revision(
        &self,
        entity_type: &str,
        id: &str,
        device_id: &str,
        deleted: bool,
    ) -> Result<String, String>;
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum LocalUsageWrite {
    UpsertSession(UsageSession),
    DeleteSession(String),
    UpsertEvent(UsageEvent),
    DeleteEvent(String),
    UpsertSpan(UsageSpanWrite),
}

#[derive(Debug, Clone)]
pub struct UsageMutation {
    pub entity_type: &'static str,
    pub id: String,
    pub data: Value,
    pub hlc: String,
    pub deleted: Option<bool>,
}

pub fn apply_local_usage_write(
    session: &dyn UsageWriteSession,
    write_device_id: &str,
    write: LocalUsageWrite,
) -> Result<Vec<UsageMutation>, String> {
    let sequence_device_id = match &write {
        LocalUsageWrite::UpsertSpan(span) => span.device_id.as_str(),
        _ => write_device_id,
    };
    session.ensure_sequence_migrated(sequence_device_id)?;

    match write {
        LocalUsageWrite::UpsertSession(value) => {
            session.upsert_session(&value)?;
            let id = value.id.clone();
            let hlc = session.record_revision("usage_session", &id, write_device_id, false)?;
            Ok(vec![mutation("usage_session", &id, value, hlc, None)?])
        }
        LocalUsageWrite::DeleteSession(id) => {
            session.delete_session(&id)?;
            let hlc = session.record_revision("usage_session", &id, write_device_id, true)?;
            Ok(vec![deleted_mutation("usage_session", id, hlc)])
        }
        LocalUsageWrite::UpsertEvent(value) => {
            session.upsert_event(&value)?;
            let id = value.id.clone();
            let hlc = session.record_revision("usage_event", &id, write_device_id, false)?;
            Ok(vec![mutation("usage_event", &id, value, hlc, None)?])
        }
        LocalUsageWrite::DeleteEvent(id) => {
            session.delete_event(&id)?;
            let hlc = session.record_revision("usage_event", &id, write_device_id, true)?;
            Ok(vec![deleted_mutation("usage_event", id, hlc)])
        }
        LocalUsageWrite::UpsertSpan(span) => session
            .upsert_span(&span)?
            .into_iter()
            .map(|day| {
                let id = day.id.clone();
                let hlc = session.record_revision("usage_day", &id, &span.device_id, false)?;
                mutation("usage_day", &id, day, hlc, None)
            })
            .collect(),
    }
}

fn mutation<T: serde::Serialize>(
    entity_type: &'static str,
    id: &str,
    value: T,
    hlc: String,
    deleted: Option<bool>,
) -> Result<UsageMutation, String> {
    Ok(UsageMutation {
        entity_type,
        id: id.to_owned(),
        data: serde_json::to_value(value).map_err(|error| error.to_string())?,
        hlc,
        deleted,
    })
}

fn deleted_mutation(entity_type: &'static str, id: String, hlc: String) -> UsageMutation {
    UsageMutation {
        entity_type,
        id,
        data: serde_json::json!({}),
        hlc,
        deleted: Some(true),
    }
}
