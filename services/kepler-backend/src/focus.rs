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
/// focus-логику без запуска ark-core-rpc child-процесса.
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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActiveState {
    pub active: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocklist_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
}

impl Default for ActiveState {
    fn default() -> Self {
        Self {
            active: false,
            blocklist_id: None,
            started_at: None,
        }
    }
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
        let ok = b.is_ascii_lowercase()
            || b.is_ascii_digit()
            || b == b'.'
            || b == b'-';
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

fn validate_domains(raw: &[String]) -> Result<Vec<String>, String> {
    let mut out = Vec::with_capacity(raw.len());
    for d in raw {
        out.push(validate_domain(d)?);
    }
    Ok(out)
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
/// дёшево: `INSERT OR REPLACE`, и оставляет invariant'у "type существует"
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
    Some(Blocklist {
        id,
        name,
        domains,
        created_at,
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

#[derive(Debug, Deserialize)]
pub struct UpsertParams {
    #[serde(default)]
    pub id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub domains: Vec<String>,
}

pub async fn upsert_blocklist<R: FocusArkRequester>(
    ark: &R,
    params: UpsertParams,
) -> Result<Blocklist, String> {
    let name = params.name.trim().to_string();
    if name.is_empty() {
        return Err("name must be non-empty".into());
    }
    let domains = validate_domains(&params.domains)?;

    ensure_blocklist_object_type(ark).await?;

    let (id, created_at) = if let Some(id) = params.id.clone() {
        // Fetch existing, чтобы сохранить createdAt.
        let existing = ark
            .request("get_object", json!({ "id": id }))
            .await
            .ok();
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
    })
}

pub async fn delete_blocklist<R: FocusArkRequester>(ark: &R, id: &str) -> Result<(), String> {
    ensure_blocklist_object_type(ark).await?;
    // Fetch existing — чтобы сохранить props + content при soft-delete.
    let existing = ark
        .request("get_object", json!({ "id": id }))
        .await?;
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
}

pub async fn set_active_state<R: FocusArkRequester>(
    ark: &R,
    params: SetActiveStateParams,
) -> Result<(), String> {
    let state = if params.active {
        ActiveState {
            active: true,
            blocklist_id: params.blocklist_id,
            started_at: Some(now_iso()),
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
                        .filter(|v| {
                            v.get("typeId").and_then(|t| t.as_str()) == Some(type_id)
                        })
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
            },
        )
        .await
        .unwrap();

        let listed = list_blocklists(&ark).await.unwrap();
        assert_eq!(listed.len(), 2);

        delete_blocklist(&ark, &a.id).await.unwrap();
        let listed_after = list_blocklists(&ark).await.unwrap();
        assert_eq!(listed_after.len(), 1);
        assert_eq!(listed_after[0].name, "B");
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
            },
        )
        .await
        .unwrap();
        let state = get_active_state(&ark).await.unwrap();
        assert!(state.active);
        assert_eq!(state.blocklist_id.as_deref(), Some("bl-1"));
        assert!(state.started_at.is_some());

        set_active_state(
            &ark,
            SetActiveStateParams {
                active: false,
                blocklist_id: None,
            },
        )
        .await
        .unwrap();
        let state = get_active_state(&ark).await.unwrap();
        assert!(!state.active);
        assert!(state.blocklist_id.is_none());
        assert!(state.started_at.is_none());
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
