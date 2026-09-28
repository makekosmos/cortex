// Focus-mode блоклисты для Kepler.
//
// Хранение:
//   - `blocklist_obj` — ARK object_type (lazy-зарегистрирован на первом вызове
//     focus.*). Props: `{ name, domains, createdAt }`. Soft-delete через
//     `deletedAt` патч на object.
//   - Active state — `sync_kv` под key `focus.active_state`, JSON-encoded
//     `{ active, blocklist_id?, started_at? }`.
//
// Применение блокировки (hosts file write) делает shell — этот модуль
// **только** хранит state. Никаких helper bin / privileged ops.
//
// WS-операции (см. dispatch в ws_server.rs):
//   - `focus.list_blocklists`
//   - `focus.upsert_blocklist`
//   - `focus.delete_blocklist`
//   - `focus.get_active_state`
//   - `focus.set_active_state`

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::ark_host::ArkHost;

pub const BLOCKLIST_OBJ_TYPE_ID: &str = "blocklist_obj";
pub const FOCUS_ACTIVE_STATE_KEY: &str = "focus.active_state";

/// Минимальный async-trait для ARK операций, нужных focus-модулю.
///
/// Реальный `ArkHost` имплементит этот trait через blanket impl ниже.
/// Тесты подсовывают `FakeArk` с in-memory state — это позволяет тестировать
/// focus-логику без запуска ARK-сервиса.
#[async_trait]
pub trait FocusArkRequester: Send + Sync {
    async fn request(&self, operation: &str, params: Value) -> Result<Value, String>;
}

#[async_trait]
impl FocusArkRequester for ArkHost {
    async fn request(&self, operation: &str, params: Value) -> Result<Value, String> {
        let resp = ArkHost::request(self, operation, params)
            .await
            .map_err(|e| e.to_string())?;
        if !resp.ok {
            return Err(resp.error.unwrap_or_else(|| "ark_host: unknown".into()));
        }
        Ok(resp.data)
    }
}

// ---------- Response shapes ----------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Blocklist {
    pub id: String,
    pub name: String,
    pub domains: Vec<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    /// Built-in preset (re-created on startup if missing, unless soft-deleted).
    #[serde(default)]
    pub preset: bool,
    /// Emoji/icon shorthand. Empty if not set.
    #[serde(default)]
    pub icon: String,
    /// "domains" — strictly parsed domain list; "raw" — список может содержать
    /// `@list-id` references (резолвится через `focus.resolve_blocklist_domains`).
    #[serde(default = "default_kind")]
    pub kind: String,
}

fn default_kind() -> String {
    "domains".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BlockedApp {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ActiveState {
    pub active: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocklist_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocked_app_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocked_apps: Vec<BlockedApp>,
}

// ---------- Validation ----------

/// Валидация одного домена. Returns trimmed lowercase form on success.
///
/// Правило: `/^[a-z0-9.-]+\.[a-z]{2,}$/` (case-insensitive). Trim'им до
/// валидации, lowercase'им результат.
pub fn validate_domain(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("domain cannot be empty".into());
    }
    let lower = trimmed.to_ascii_lowercase();
    let bytes = lower.as_bytes();

    // Все символы — [a-z0-9.-].
    for &b in bytes {
        let ok = b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-';
        if !ok {
            return Err(format!("invalid character in domain {raw:?}"));
        }
    }

    // Должен содержать хотя бы одну точку и TLD длиной >=2 буквы.
    let last_dot = match lower.rfind('.') {
        Some(i) => i,
        None => return Err(format!("domain {raw:?} has no TLD")),
    };
    if last_dot == 0 || last_dot == lower.len() - 1 {
        return Err(format!("domain {raw:?} has empty label adjacent to dot"));
    }
    let tld = &lower[last_dot + 1..];
    if tld.len() < 2 || !tld.bytes().all(|b| b.is_ascii_lowercase()) {
        return Err(format!("domain {raw:?} TLD must be >=2 letters"));
    }
    // Label слева тоже должен быть непустой.
    let head = &lower[..last_dot];
    if head.split('.').any(|label| label.is_empty()) {
        return Err(format!("domain {raw:?} has empty label"));
    }

    Ok(lower)
}

// ---------- Object_type registration (lazy, idempotent) ----------

fn blocklist_object_type_definition() -> Value {
    let schema = json!({
        "fields": [
            { "id": "name", "label": "Название", "kind": "text", "required": true, "visible": true, "read_only": false },
            { "id": "domains", "label": "Домены", "kind": "text", "required": false, "visible": true, "read_only": false },
            { "id": "createdAt", "label": "Создан", "kind": "date", "required": false, "visible": true, "read_only": true }
        ]
    });
    let ui_schema = json!({
        "visible_fields": ["name", "domains", "createdAt"],
        "hidden_fields": ["created_at", "updated_at", "deleted_at"],
        "read_only_fields": ["createdAt"],
    });
    let epoch = "1970-01-01T00:00:00.000Z";
    json!({
        "id": BLOCKLIST_OBJ_TYPE_ID,
        "name": "Focus blocklist",
        "schemaJson": schema.to_string(),
        "uiSchemaJson": ui_schema.to_string(),
        "createdAt": epoch,
        "updatedAt": epoch,
        "systemLocked": false,
    })
}

/// Идемпотентно регистрирует `blocklist_obj` object_type в ARK.
///
/// Вызывается при каждой focus.* операции (а не только при первой) — это
/// дёшево: `upsert_object_type`, и оставляет invariant'у "type существует"
/// для любого вызова. Альтернатива (OnceCell) — лишнее состояние на модуле,
/// сложнее testability.
pub async fn ensure_blocklist_object_type<R: FocusArkRequester>(ark: &R) -> Result<(), String> {
    ark.request(
        "upsert_object_type",
        json!({ "object_type": blocklist_object_type_definition() }),
    )
    .await
    .map(|_| ())
}

// ---------- Handlers ----------

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn blocklist_from_ark_object(obj: &Value) -> Option<Blocklist> {
    let id = obj.get("id").and_then(|v| v.as_str())?.to_string();
    let props = obj.get("propsJson").cloned().unwrap_or(json!({}));
    let name = props
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let domains: Vec<String> = props
        .get("domains")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    let created_at = props
        .get("createdAt")
        .and_then(|v| v.as_str())
        .or_else(|| obj.get("createdAt").and_then(|v| v.as_str()))
        .unwrap_or("")
        .to_string();
    let preset = props
        .get("preset")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let icon = props
        .get("icon")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let kind = props
        .get("kind")
        .and_then(|v| v.as_str())
        .unwrap_or("domains")
        .to_string();
    Some(Blocklist {
        id,
        name,
        domains,
        created_at,
        preset,
        icon,
        kind,
    })
}

fn is_soft_deleted(obj: &Value) -> bool {
    matches!(
        obj.get("deletedAt"),
        Some(v) if !v.is_null()
    )
}

pub async fn list_blocklists<R: FocusArkRequester>(ark: &R) -> Result<Vec<Blocklist>, String> {
    ensure_blocklist_object_type(ark).await?;
    ensure_presets(ark).await?;
    let raw = ark
        .request(
            "list_objects_by_type",
            json!({ "type_id": BLOCKLIST_OBJ_TYPE_ID }),
        )
        .await?;
    let arr = raw.as_array().cloned().unwrap_or_default();
    let mut out: Vec<Blocklist> = Vec::with_capacity(arr.len());
    for obj in &arr {
        if is_soft_deleted(obj) {
            continue;
        }
        if let Some(b) = blocklist_from_ark_object(obj) {
            out.push(b);
        }
    }
    Ok(out)
}

// ---------- Presets ----------

/// Описание built-in пресета.
struct PresetDef {
    id: &'static str,
    name: &'static str,
    icon: &'static str,
    domains: &'static [&'static str],
}

const PRESETS: &[PresetDef] = &[
    PresetDef {
        id: "preset:distractions",
        name: "Distractions",
        icon: "🛡️",
        domains: &[
            "tiktok.com",
            "www.tiktok.com",
            "twitter.com",
            "www.twitter.com",
            "x.com",
            "www.x.com",
            "reddit.com",
            "www.reddit.com",
            "youtube.com",
            "www.youtube.com",
            "instagram.com",
            "www.instagram.com",
            "facebook.com",
            "www.facebook.com",
        ],
    },
    PresetDef {
        id: "preset:news",
        name: "News",
        icon: "📰",
        domains: &[
            "news.ycombinator.com",
            "techcrunch.com",
            "theverge.com",
            "arstechnica.com",
        ],
    },
];

/// Idempotent: вставляет недостающие пресеты. Если объект существует (даже
/// soft-deleted) — пропускаем. Юзер мог удалить preset или отредактировать
/// его — мы не перезаписываем.
pub async fn ensure_presets<R: FocusArkRequester>(ark: &R) -> Result<(), String> {
    for def in PRESETS {
        // Проверяем — есть ли уже объект с таким id (любой, включая soft-deleted).
        let existing = ark
            .request("get_object", json!({ "id": def.id }))
            .await
            .ok();
        if existing.is_some() {
            continue;
        }
        let now = now_iso();
        let domains: Vec<String> = def.domains.iter().map(|s| (*s).to_string()).collect();
        let props = json!({
            "name": def.name,
            "domains": domains,
            "createdAt": now,
            "preset": true,
            "icon": def.icon,
            "kind": "domains",
        });
        let object = json!({
            "id": def.id,
            "typeId": BLOCKLIST_OBJ_TYPE_ID,
            "title": def.name,
            "contentJson": {},
            "propsJson": props,
            "createdAt": now,
            "updatedAt": now,
            "deletedAt": null,
        });
        ark.request("upsert_object", json!({ "object": object }))
            .await?;
    }
    Ok(())
}

// ---------- Resolve `@references` ----------

/// Рекурсивно разворачивает `@list-id` references в плоский deduped список
/// доменов. Cycle detection через `visited` set.
pub async fn resolve_blocklist_domains<R: FocusArkRequester>(
    ark: &R,
    id: &str,
) -> Result<Vec<String>, String> {
    ensure_blocklist_object_type(ark).await?;
    let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut out: Vec<String> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    resolve_recursive(ark, id, &mut visited, &mut out, &mut seen).await?;
    Ok(out)
}

fn resolve_recursive<'a, R: FocusArkRequester>(
    ark: &'a R,
    id: &'a str,
    visited: &'a mut std::collections::HashSet<String>,
    out: &'a mut Vec<String>,
    seen: &'a mut std::collections::HashSet<String>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send + 'a>> {
    Box::pin(async move {
        if !visited.insert(id.to_string()) {
            return Err(format!("cycle detected at blocklist {id:?}"));
        }
        let obj = ark
            .request("get_object", json!({ "id": id }))
            .await
            .map_err(|e| format!("get_object({id}): {e}"))?;
        if is_soft_deleted(&obj) {
            return Err(format!("blocklist {id:?} is deleted"));
        }
        let bl = blocklist_from_ark_object(&obj)
            .ok_or_else(|| format!("blocklist {id:?}: invalid shape"))?;
        for entry in &bl.domains {
            if let Some(ref_id) = entry.strip_prefix('@') {
                resolve_recursive(ark, ref_id, visited, out, seen).await?;
            } else if seen.insert(entry.clone()) {
                out.push(entry.clone());
            }
        }
        Ok(())
    })
}

#[derive(Debug, Deserialize, Default)]
pub struct UpsertParams {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub domains: Vec<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub preset: Option<bool>,
}

/// Validate domain entries with optional `@reference` support for raw kind.
fn validate_entries(raw: &[String], kind: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::with_capacity(raw.len());
    for d in raw {
        let trimmed = d.trim();
        if trimmed.is_empty() {
            continue;
        }
        if kind == "raw" && trimmed.starts_with('@') {
            // Reference — keep as-is (validated at resolve time).
            out.push(trimmed.to_string());
        } else {
            out.push(validate_domain(trimmed)?);
        }
    }
    Ok(out)
}

pub async fn upsert_blocklist<R: FocusArkRequester>(
    ark: &R,
    params: UpsertParams,
) -> Result<Blocklist, String> {
    let name = params.name.trim().to_string();
    if name.is_empty() {
        return Err("name must be non-empty".into());
    }
    let kind = params.kind.clone().unwrap_or_else(|| "domains".into());
    if kind != "domains" && kind != "raw" {
        return Err(format!("unknown kind {kind:?}"));
    }
    let domains = validate_entries(&params.domains, &kind)?;
    let icon = params.icon.clone().unwrap_or_default();
    let preset_flag = params.preset.unwrap_or(false);

    ensure_blocklist_object_type(ark).await?;

    let (id, created_at) = if let Some(id) = params.id.clone() {
        // Fetch existing, чтобы сохранить createdAt.
        let existing = ark.request("get_object", json!({ "id": id })).await.ok();
        let created_at = existing
            .as_ref()
            .and_then(|v| v.get("propsJson"))
            .and_then(|p| p.get("createdAt"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(now_iso);
        (id, created_at)
    } else {
        (uuid::Uuid::new_v4().to_string(), now_iso())
    };

    let now = now_iso();
    let props = json!({
        "name": name,
        "domains": domains,
        "createdAt": created_at,
        "preset": preset_flag,
        "icon": icon,
        "kind": kind,
    });
    let object = json!({
        "id": id,
        "typeId": BLOCKLIST_OBJ_TYPE_ID,
        "title": name,
        "contentJson": {},
        "propsJson": props,
        "createdAt": created_at,
        "updatedAt": now,
        "deletedAt": null,
    });
    ark.request("upsert_object", json!({ "object": object }))
        .await?;

    Ok(Blocklist {
        id,
        name,
        domains,
        created_at,
        preset: preset_flag,
        icon,
        kind,
    })
}

pub async fn delete_blocklist<R: FocusArkRequester>(ark: &R, id: &str) -> Result<(), String> {
    ensure_blocklist_object_type(ark).await?;
    // Fetch existing — чтобы сохранить props + content при soft-delete.
    let existing = ark.request("get_object", json!({ "id": id })).await?;
    let now = now_iso();
    let mut patched = match existing {
        Value::Object(map) => map,
        _ => return Err(format!("get_object: bad shape for id={id}")),
    };
    patched.insert("deletedAt".into(), Value::String(now.clone()));
    patched.insert("updatedAt".into(), Value::String(now));
    ark.request("upsert_object", json!({ "object": Value::Object(patched) }))
        .await?;
    Ok(())
}

pub async fn get_active_state<R: FocusArkRequester>(ark: &R) -> Result<ActiveState, String> {
    let raw = ark
        .request("get_sync_kv", json!({ "key": FOCUS_ACTIVE_STATE_KEY }))
        .await?;
    // get_sync_kv возвращает Option<String> (json-encoded или null).
    let encoded = match raw {
        Value::Null => return Ok(ActiveState::default()),
        Value::String(s) => s,
        // Если сервер вернёт объект {value: "..."} — fallback. Сейчас тип
        // Option<String> в RPC, но мы tolerant к обёртке.
        Value::Object(ref map) => match map.get("value") {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Null) | None => return Ok(ActiveState::default()),
            Some(other) => other.to_string(),
        },
        other => other.to_string(),
    };
    if encoded.is_empty() {
        return Ok(ActiveState::default());
    }
    serde_json::from_str::<ActiveState>(&encoded)
        .map_err(|e| format!("focus.active_state: parse '{encoded}': {e}"))
}

#[derive(Debug, Deserialize)]
pub struct SetActiveStateParams {
    pub active: bool,
    #[serde(default)]
    pub blocklist_id: Option<String>,
    #[serde(default)]
    pub blocked_app_ids: Vec<String>,
    #[serde(default)]
    pub blocked_apps: Vec<BlockedApp>,
}

pub async fn set_active_state<R: FocusArkRequester>(
    ark: &R,
    params: SetActiveStateParams,
) -> Result<(), String> {
    let state = if params.active {
        let blocked_apps: Vec<BlockedApp> = params
            .blocked_apps
            .into_iter()
            .filter_map(|app| {
                let id = app.id.trim().to_string();
                let name = app.name.trim().to_string();
                if id.is_empty() || name.is_empty() {
                    return None;
                }
                Some(BlockedApp {
                    id,
                    name,
                    icon: app.icon.filter(|icon| !icon.trim().is_empty()),
                })
            })
            .collect();
        ActiveState {
            active: true,
            blocklist_id: params.blocklist_id,
            started_at: Some(now_iso()),
            blocked_app_ids: params
                .blocked_app_ids
                .into_iter()
                .filter(|id| !id.trim().is_empty())
                .collect(),
            blocked_apps,
        }
    } else {
        ActiveState::default()
    };
    let encoded = serde_json::to_string(&state)
        .map_err(|e| format!("focus.set_active_state: serialize: {e}"))?;
    ark.request(
        "set_sync_kv",
        json!({ "key": FOCUS_ACTIVE_STATE_KEY, "value": encoded }),
    )
    .await?;
    Ok(())
}

// ---------- WS dispatch ----------

/// Тип response для dispatch'а — соответствует форме `LocalResponse` в
/// `ws_server.rs` (но определён здесь чтобы избежать циклической зависимости).
pub struct FocusResponse {
    pub ok: bool,
    pub data: Value,
    pub error: Option<String>,
}

impl FocusResponse {
    fn ok(data: Value) -> Self {
        Self {
            ok: true,
            data,
            error: None,
        }
    }
    fn err(msg: impl Into<String>) -> Self {
        Self {
            ok: false,
            data: Value::Null,
            error: Some(msg.into()),
        }
    }
}

pub async fn handle_focus_op(subop: &str, params: Value, ark: &ArkHost) -> FocusResponse {
    match subop {
        "list_blocklists" => match list_blocklists(ark).await {
            Ok(list) => FocusResponse::ok(json!({ "blocklists": list })),
            Err(e) => FocusResponse::err(format!("focus.list_blocklists: {e}")),
        },
        "upsert_blocklist" => {
            let p: UpsertParams = match serde_json::from_value(params) {
                Ok(p) => p,
                Err(e) => {
                    return FocusResponse::err(format!("focus.upsert_blocklist: params: {e}"))
                }
            };
            match upsert_blocklist(ark, p).await {
                Ok(b) => match serde_json::to_value(&b) {
                    Ok(v) => FocusResponse::ok(v),
                    Err(e) => FocusResponse::err(format!("focus.upsert_blocklist: serialize: {e}")),
                },
                Err(e) => FocusResponse::err(format!("focus.upsert_blocklist: {e}")),
            }
        }
        "delete_blocklist" => {
            let id = match params.get("id").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => return FocusResponse::err("focus.delete_blocklist: missing 'id'"),
            };
            match delete_blocklist(ark, &id).await {
                Ok(()) => FocusResponse::ok(json!({ "ok": true })),
                Err(e) => FocusResponse::err(format!("focus.delete_blocklist: {e}")),
            }
        }
        "get_active_state" => match get_active_state(ark).await {
            Ok(state) => match serde_json::to_value(&state) {
                Ok(v) => FocusResponse::ok(v),
                Err(e) => FocusResponse::err(format!("focus.get_active_state: serialize: {e}")),
            },
            Err(e) => FocusResponse::err(format!("focus.get_active_state: {e}")),
        },
        "set_active_state" => {
            let p: SetActiveStateParams = match serde_json::from_value(params) {
                Ok(p) => p,
                Err(e) => {
                    return FocusResponse::err(format!("focus.set_active_state: params: {e}"))
                }
            };
            match set_active_state(ark, p).await {
                Ok(()) => FocusResponse::ok(json!({ "ok": true })),
                Err(e) => FocusResponse::err(format!("focus.set_active_state: {e}")),
            }
        }
        "ensure_presets" => match ensure_presets(ark).await {
            Ok(()) => FocusResponse::ok(json!({ "ok": true })),
            Err(e) => FocusResponse::err(format!("focus.ensure_presets: {e}")),
        },
        "resolve_blocklist_domains" => {
            let id = match params.get("id").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => return FocusResponse::err("focus.resolve_blocklist_domains: missing 'id'"),
            };
            match resolve_blocklist_domains(ark, &id).await {
                Ok(domains) => FocusResponse::ok(json!({ "domains": domains })),
                Err(e) => FocusResponse::err(format!("focus.resolve_blocklist_domains: {e}")),
            }
        }
        other => FocusResponse::err(format!("focus.{other}: unknown sub-operation")),
    }
}

// ---------- Tests ----------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// In-memory `FocusArkRequester` для тестов: имитирует `objects` table
    /// (упрощённая модель) + sync_kv. Не дёргает реальный ARK.
    struct FakeArk {
        objects: Mutex<std::collections::HashMap<String, Value>>,
        object_types: Mutex<std::collections::HashMap<String, Value>>,
        sync_kv: Mutex<std::collections::HashMap<String, String>>,
    }

    impl FakeArk {
        fn new() -> Self {
            Self {
                objects: Mutex::new(std::collections::HashMap::new()),
                object_types: Mutex::new(std::collections::HashMap::new()),
                sync_kv: Mutex::new(std::collections::HashMap::new()),
            }
        }
    }

    #[async_trait]
    impl FocusArkRequester for FakeArk {
        async fn request(&self, operation: &str, params: Value) -> Result<Value, String> {
            match operation {
                "upsert_object_type" => {
                    let ot = params
                        .get("object_type")
                        .cloned()
                        .ok_or("missing object_type")?;
                    let id = ot
                        .get("id")
                        .and_then(|v| v.as_str())
                        .ok_or("object_type.id missing")?
                        .to_string();
                    self.object_types.lock().unwrap().insert(id, ot);
                    Ok(json!(true))
                }
                "get_object" => {
                    let id = params
                        .get("id")
                        .and_then(|v| v.as_str())
                        .ok_or("missing id")?;
                    let map = self.objects.lock().unwrap();
                    map.get(id)
                        .cloned()
                        .ok_or_else(|| format!("object {id} not found"))
                }
                "upsert_object" => {
                    let obj = params.get("object").cloned().ok_or("missing object")?;
                    let id = obj
                        .get("id")
                        .and_then(|v| v.as_str())
                        .ok_or("object.id missing")?
                        .to_string();
                    self.objects.lock().unwrap().insert(id, obj);
                    Ok(json!(true))
                }
                "list_objects_by_type" => {
                    let type_id = params
                        .get("type_id")
                        .and_then(|v| v.as_str())
                        .ok_or("missing type_id")?;
                    let map = self.objects.lock().unwrap();
                    let arr: Vec<Value> = map
                        .values()
                        .filter(|v| v.get("typeId").and_then(|t| t.as_str()) == Some(type_id))
                        .cloned()
                        .collect();
                    Ok(Value::Array(arr))
                }
                "get_sync_kv" => {
                    let key = params
                        .get("key")
                        .and_then(|v| v.as_str())
                        .ok_or("missing key")?;
                    let map = self.sync_kv.lock().unwrap();
                    Ok(match map.get(key) {
                        Some(s) => Value::String(s.clone()),
                        None => Value::Null,
                    })
                }
                "set_sync_kv" => {
                    let key = params
                        .get("key")
                        .and_then(|v| v.as_str())
                        .ok_or("missing key")?
                        .to_string();
                    let value = params
                        .get("value")
                        .and_then(|v| v.as_str())
                        .ok_or("missing value")?
                        .to_string();
                    self.sync_kv.lock().unwrap().insert(key, value);
                    Ok(json!(true))
                }
                other => Err(format!("unknown op {other}")),
            }
        }
    }

    // ---- Validation ----

    #[test]
    fn validate_domain_accepts_typical_forms() {
        assert_eq!(validate_domain("example.com").unwrap(), "example.com");
        assert_eq!(validate_domain("EXAMPLE.COM").unwrap(), "example.com");
        assert_eq!(validate_domain("  reddit.com  ").unwrap(), "reddit.com");
        assert_eq!(
            validate_domain("sub.domain.co.uk").unwrap(),
            "sub.domain.co.uk"
        );
        assert_eq!(
            validate_domain("my-site.example.io").unwrap(),
            "my-site.example.io"
        );
        assert_eq!(validate_domain("a1.b2c.dev").unwrap(), "a1.b2c.dev");
    }

    #[test]
    fn validate_domain_rejects_invalid_forms() {
        assert!(validate_domain("").is_err());
        assert!(validate_domain("   ").is_err());
        assert!(validate_domain("no-tld").is_err());
        assert!(validate_domain("trailing-dot.").is_err());
        assert!(validate_domain(".leading-dot.com").is_err());
        assert!(validate_domain("double..dot.com").is_err());
        assert!(validate_domain("space in.com").is_err());
        assert!(validate_domain("under_score.com").is_err());
        assert!(validate_domain("foo.bar/path").is_err());
        assert!(validate_domain("https://example.com").is_err());
        assert!(validate_domain("foo.1").is_err()); // numeric TLD
        assert!(validate_domain("foo.a").is_err()); // 1-char TLD
    }

    // ---- Upsert ----

    #[tokio::test]
    async fn upsert_new_assigns_id_and_created_at() {
        let ark = FakeArk::new();
        let result = upsert_blocklist(
            &ark,
            UpsertParams {
                id: None,
                name: "Soc nets".into(),
                domains: vec!["twitter.com".into(), "Reddit.com".into()],
                icon: None,
                kind: None,
                preset: None,
            },
        )
        .await
        .unwrap();

        assert!(!result.id.is_empty());
        // UUID v4 — 36 chars with hyphens.
        assert_eq!(result.id.len(), 36);
        assert!(!result.created_at.is_empty());
        assert_eq!(result.name, "Soc nets");
        // Domains lowercased.
        assert_eq!(result.domains, vec!["twitter.com", "reddit.com"]);

        // Persisted: object_type зарегистрирован.
        assert!(ark
            .object_types
            .lock()
            .unwrap()
            .contains_key(BLOCKLIST_OBJ_TYPE_ID));
        // Persisted: object записан.
        assert_eq!(ark.objects.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn upsert_existing_preserves_id_and_created_at() {
        let ark = FakeArk::new();
        let first = upsert_blocklist(
            &ark,
            UpsertParams {
                id: None,
                name: "Initial".into(),
                domains: vec!["x.com".into()],
                icon: None,
                kind: None,
                preset: None,
            },
        )
        .await
        .unwrap();

        let updated = upsert_blocklist(
            &ark,
            UpsertParams {
                id: Some(first.id.clone()),
                name: "Renamed".into(),
                domains: vec!["x.com".into(), "y.com".into()],
                icon: None,
                kind: None,
                preset: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(updated.id, first.id);
        assert_eq!(updated.created_at, first.created_at);
        assert_eq!(updated.name, "Renamed");
        assert_eq!(updated.domains, vec!["x.com", "y.com"]);
        // Один объект в storage.
        assert_eq!(ark.objects.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn upsert_rejects_empty_name() {
        let ark = FakeArk::new();
        let err = upsert_blocklist(
            &ark,
            UpsertParams {
                id: None,
                name: "   ".into(),
                domains: vec!["x.com".into()],
                icon: None,
                kind: None,
                preset: None,
            },
        )
        .await
        .unwrap_err();
        assert!(err.contains("name"));
    }

    #[tokio::test]
    async fn upsert_rejects_invalid_domain() {
        let ark = FakeArk::new();
        let err = upsert_blocklist(
            &ark,
            UpsertParams {
                id: None,
                name: "Test".into(),
                domains: vec!["valid.com".into(), "not a domain".into()],
                icon: None,
                kind: None,
                preset: None,
            },
        )
        .await
        .unwrap_err();
        assert!(err.contains("invalid") || err.contains("domain"));
    }

    #[tokio::test]
    async fn upsert_accepts_empty_domains() {
        let ark = FakeArk::new();
        let result = upsert_blocklist(
            &ark,
            UpsertParams {
                id: None,
                name: "Empty list".into(),
                domains: vec![],
                icon: None,
                kind: None,
                preset: None,
            },
        )
        .await
        .unwrap();
        assert!(result.domains.is_empty());
    }

    // ---- List / soft-delete ----

    #[tokio::test]
    async fn list_filters_out_soft_deleted() {
        let ark = FakeArk::new();
        let a = upsert_blocklist(
            &ark,
            UpsertParams {
                id: None,
                name: "A".into(),
                domains: vec!["a.com".into()],
                icon: None,
                kind: None,
                preset: None,
            },
        )
        .await
        .unwrap();
        let _b = upsert_blocklist(
            &ark,
            UpsertParams {
                id: None,
                name: "B".into(),
                domains: vec!["b.com".into()],
                icon: None,
                kind: None,
                preset: None,
            },
        )
        .await
        .unwrap();

        let listed = list_blocklists(&ark).await.unwrap();
        // list_blocklists auto-creates two preset rows on first call.
        let user_listed: Vec<&Blocklist> = listed.iter().filter(|b| !b.preset).collect();
        assert_eq!(user_listed.len(), 2);

        delete_blocklist(&ark, &a.id).await.unwrap();
        let listed_after = list_blocklists(&ark).await.unwrap();
        let user_after: Vec<&Blocklist> = listed_after.iter().filter(|b| !b.preset).collect();
        assert_eq!(user_after.len(), 1);
        assert_eq!(user_after[0].name, "B");
    }

    // ---- Active state ----

    #[tokio::test]
    async fn get_active_state_default_when_empty() {
        let ark = FakeArk::new();
        let state = get_active_state(&ark).await.unwrap();
        assert_eq!(state, ActiveState::default());
        assert!(!state.active);
    }

    #[tokio::test]
    async fn set_then_get_active_state_round_trip() {
        let ark = FakeArk::new();
        set_active_state(
            &ark,
            SetActiveStateParams {
                active: true,
                blocklist_id: Some("bl-1".into()),
                blocked_app_ids: vec!["steam".into(), "".into(), "discord".into()],
                blocked_apps: vec![
                    BlockedApp {
                        id: "steam".into(),
                        name: "Steam".into(),
                        icon: None,
                    },
                    BlockedApp {
                        id: "".into(),
                        name: "Ignored".into(),
                        icon: None,
                    },
                    BlockedApp {
                        id: "discord".into(),
                        name: "".into(),
                        icon: None,
                    },
                ],
            },
        )
        .await
        .unwrap();
        let state = get_active_state(&ark).await.unwrap();
        assert!(state.active);
        assert_eq!(state.blocklist_id.as_deref(), Some("bl-1"));
        assert_eq!(state.blocked_app_ids, vec!["steam", "discord"]);
        assert_eq!(
            state.blocked_apps,
            vec![BlockedApp {
                id: "steam".into(),
                name: "Steam".into(),
                icon: None,
            }]
        );
        assert!(state.started_at.is_some());

        set_active_state(
            &ark,
            SetActiveStateParams {
                active: false,
                blocklist_id: None,
                blocked_app_ids: vec!["steam".into()],
                blocked_apps: vec![BlockedApp {
                    id: "steam".into(),
                    name: "Steam".into(),
                    icon: None,
                }],
            },
        )
        .await
        .unwrap();
        let state = get_active_state(&ark).await.unwrap();
        assert!(!state.active);
        assert!(state.blocklist_id.is_none());
        assert!(state.blocked_app_ids.is_empty());
        assert!(state.blocked_apps.is_empty());
        assert!(state.started_at.is_none());
    }

    // ---- Presets ----

    #[tokio::test]
    async fn ensure_presets_creates_both_when_missing() {
        let ark = FakeArk::new();
        ensure_presets(&ark).await.unwrap();
        let objs = ark.objects.lock().unwrap();
        assert!(objs.contains_key("preset:distractions"));
        assert!(objs.contains_key("preset:news"));
        assert_eq!(objs.len(), 2);
        // Both flagged as preset, with icons.
        let d = objs.get("preset:distractions").unwrap();
        assert_eq!(
            d.get("propsJson").unwrap().get("preset").unwrap(),
            &json!(true)
        );
        assert_eq!(
            d.get("propsJson").unwrap().get("icon").unwrap(),
            &json!("🛡️")
        );
    }

    #[tokio::test]
    async fn ensure_presets_idempotent() {
        let ark = FakeArk::new();
        ensure_presets(&ark).await.unwrap();
        ensure_presets(&ark).await.unwrap();
        ensure_presets(&ark).await.unwrap();
        let objs = ark.objects.lock().unwrap();
        assert_eq!(objs.len(), 2);
    }

    #[tokio::test]
    async fn ensure_presets_skips_soft_deleted() {
        let ark = FakeArk::new();
        ensure_presets(&ark).await.unwrap();
        // Soft-delete preset:distractions.
        delete_blocklist(&ark, "preset:distractions").await.unwrap();
        // Re-run — must NOT re-create.
        ensure_presets(&ark).await.unwrap();
        let objs = ark.objects.lock().unwrap();
        let d = objs.get("preset:distractions").unwrap();
        assert!(d.get("deletedAt").is_some_and(|v| !v.is_null()));
    }

    #[tokio::test]
    async fn ensure_presets_does_not_overwrite_user_edits() {
        let ark = FakeArk::new();
        ensure_presets(&ark).await.unwrap();
        // Simulate user editing preset (change domains).
        upsert_blocklist(
            &ark,
            UpsertParams {
                id: Some("preset:distractions".into()),
                name: "My version".into(),
                domains: vec!["custom.com".into()],
                icon: Some("🚫".into()),
                kind: None,
                preset: Some(true),
            },
        )
        .await
        .unwrap();
        // Re-run ensure — must not overwrite.
        ensure_presets(&ark).await.unwrap();
        let listed = list_blocklists(&ark).await.unwrap();
        let edited = listed
            .iter()
            .find(|b| b.id == "preset:distractions")
            .unwrap();
        assert_eq!(edited.name, "My version");
        assert_eq!(edited.domains, vec!["custom.com"]);
    }

    #[tokio::test]
    async fn list_blocklists_auto_creates_presets() {
        let ark = FakeArk::new();
        let listed = list_blocklists(&ark).await.unwrap();
        // Both presets should now appear.
        assert!(listed.iter().any(|b| b.id == "preset:distractions"));
        assert!(listed.iter().any(|b| b.id == "preset:news"));
    }

    // ---- Resolve @references ----

    #[tokio::test]
    async fn resolve_returns_plain_domains_for_simple_list() {
        let ark = FakeArk::new();
        let bl = upsert_blocklist(
            &ark,
            UpsertParams {
                id: None,
                name: "A".into(),
                domains: vec!["a.com".into(), "b.com".into()],
                icon: None,
                kind: None,
                preset: None,
            },
        )
        .await
        .unwrap();
        let resolved = resolve_blocklist_domains(&ark, &bl.id).await.unwrap();
        assert_eq!(resolved, vec!["a.com", "b.com"]);
    }

    #[tokio::test]
    async fn resolve_expands_references() {
        let ark = FakeArk::new();
        let base = upsert_blocklist(
            &ark,
            UpsertParams {
                id: None,
                name: "Base".into(),
                domains: vec!["base.com".into()],
                icon: None,
                kind: None,
                preset: None,
            },
        )
        .await
        .unwrap();
        let parent = upsert_blocklist(
            &ark,
            UpsertParams {
                id: None,
                name: "Parent".into(),
                domains: vec![format!("@{}", base.id), "extra.com".into()],
                icon: None,
                kind: Some("raw".into()),
                preset: None,
            },
        )
        .await
        .unwrap();
        let resolved = resolve_blocklist_domains(&ark, &parent.id).await.unwrap();
        assert_eq!(resolved, vec!["base.com", "extra.com"]);
    }

    #[tokio::test]
    async fn resolve_detects_cycle() {
        let ark = FakeArk::new();
        // Create A referencing B, then patch B to reference A.
        let a = upsert_blocklist(
            &ark,
            UpsertParams {
                id: Some("bl-a".into()),
                name: "A".into(),
                domains: vec!["@bl-b".into()],
                icon: None,
                kind: Some("raw".into()),
                preset: None,
            },
        )
        .await
        .unwrap();
        let _b = upsert_blocklist(
            &ark,
            UpsertParams {
                id: Some("bl-b".into()),
                name: "B".into(),
                domains: vec!["@bl-a".into()],
                icon: None,
                kind: Some("raw".into()),
                preset: None,
            },
        )
        .await
        .unwrap();
        let err = resolve_blocklist_domains(&ark, &a.id).await.unwrap_err();
        assert!(err.contains("cycle"));
    }

    #[tokio::test]
    async fn resolve_dedupes_overlapping_references() {
        let ark = FakeArk::new();
        let a = upsert_blocklist(
            &ark,
            UpsertParams {
                id: Some("bl-x".into()),
                name: "X".into(),
                domains: vec!["dup.com".into()],
                icon: None,
                kind: None,
                preset: None,
            },
        )
        .await
        .unwrap();
        let parent = upsert_blocklist(
            &ark,
            UpsertParams {
                id: None,
                name: "P".into(),
                domains: vec![format!("@{}", a.id), "dup.com".into()],
                icon: None,
                kind: Some("raw".into()),
                preset: None,
            },
        )
        .await
        .unwrap();
        let resolved = resolve_blocklist_domains(&ark, &parent.id).await.unwrap();
        assert_eq!(resolved, vec!["dup.com"]);
    }

    #[tokio::test]
    async fn ensure_object_type_idempotent() {
        let ark = FakeArk::new();
        ensure_blocklist_object_type(&ark).await.unwrap();
        ensure_blocklist_object_type(&ark).await.unwrap();
        ensure_blocklist_object_type(&ark).await.unwrap();
        let types = ark.object_types.lock().unwrap();
        assert_eq!(types.len(), 1);
        assert!(types.contains_key(BLOCKLIST_OBJ_TYPE_ID));
    }
}
