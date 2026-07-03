// Arrancador RAWG.io client — search + game details + apply metadata to game_obj.
//
// Subagent B scope: всё в этом файле + `arrancador.rawg.*` WS dispatch (см.
// `ws_server.rs`). Не трогает scanner/launcher/sqoba — это другие subagent'ы.
//
// RAWG API docs: https://api.rawg.io/docs/
//   * GET /api/games?search=<q>&key=<k>&page_size=20  → paginated list
//   * GET /api/games/{id}?key=<k>                     → detail (description, ...)
//   * GET /api/games/{id}/screenshots?key=<k>         → screenshots
//
// Storage of API key — `arrancador-config.json` (`ArrancadorConfig.rawg_api_key`).
//
// Apply metadata workflow:
//   1. fetch get_details(rawg_id)
//   2. fetch existing game_obj (`get_object`)
//   3. merge props: existing.props_json ∪ {cover_image, background_image,
//      description, genres[], platforms[], released, rawg_id}
//   4. upsert_object с обновлённым `updated_at = now()`
//
// ArkHost абстрагирован через `ArkRequester` trait (см. ниже) — позволяет
// мокировать в unit-тестах без реального ark-core-rpc child процесса.

use serde::{Deserialize, Serialize};
use thiserror::Error;

const RAWG_BASE_URL: &str = "https://api.rawg.io/api";
const USER_AGENT: &str = "Kosmos/0.1 Kepler";
const PAGE_SIZE: u32 = 20;

// ---------- Types ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawgGenre {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawgPlatform {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawgPlatformShort {
    pub platform: RawgPlatform,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawgGame {
    pub id: u32,
    pub name: String,
    pub slug: String,
    #[serde(default)]
    pub released: Option<String>,
    #[serde(default)]
    pub background_image: Option<String>,
    #[serde(default)]
    pub rating: Option<f32>,
    #[serde(default)]
    pub genres: Vec<RawgGenre>,
    #[serde(default)]
    pub platforms: Vec<RawgPlatformShort>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawgScreenshot {
    pub image: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawgGameDetail {
    pub id: u32,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub description_raw: Option<String>,
    #[serde(default)]
    pub released: Option<String>,
    #[serde(default)]
    pub background_image: Option<String>,
    #[serde(default)]
    pub genres: Vec<RawgGenre>,
    #[serde(default)]
    pub platforms: Vec<RawgPlatformShort>,
    #[serde(default)]
    pub screenshots: Vec<RawgScreenshot>,
}

#[derive(Debug, Deserialize)]
struct RawgSearchResponse {
    #[serde(default)]
    results: Vec<RawgGame>,
}

// ---------- Errors ----------

#[derive(Debug, Error)]
pub enum RawgError {
    #[error("RAWG API key not configured")]
    NoApiKey,
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("RAWG returned HTTP {0}: {1}")]
    Status(u16, String),
    #[error("failed to parse RAWG response: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("ark_host request failed: {0}")]
    ArkHost(String),
}

// ---------- HTTP client ----------

fn build_client() -> Result<reqwest::Client, RawgError> {
    Ok(reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(std::time::Duration::from_secs(30))
        .build()?)
}

/// Поиск игр по строке. Возвращает первые `PAGE_SIZE` результатов.
pub async fn search(query: &str, api_key: &str) -> Result<Vec<RawgGame>, RawgError> {
    search_with_base(query, api_key, RAWG_BASE_URL).await
}

async fn search_with_base(
    query: &str,
    api_key: &str,
    base_url: &str,
) -> Result<Vec<RawgGame>, RawgError> {
    if api_key.is_empty() {
        return Err(RawgError::NoApiKey);
    }
    let url = format!(
        "{}/games?search={}&key={}&page_size={}",
        base_url,
        urlencoding::encode(query),
        urlencoding::encode(api_key),
        PAGE_SIZE,
    );
    let client = build_client()?;
    let resp = client.get(&url).send().await?;
    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        return Err(RawgError::Status(status.as_u16(), text));
    }
    let parsed: RawgSearchResponse = serde_json::from_str(&text)?;
    Ok(parsed.results)
}

/// Детали игры (description, screenshots не fetched отдельно — мы берём из
/// `/games/{id}`, screenshots приедут пустыми если RAWG их не вернул в этом
/// ответе; апи `/screenshots` можно добавить отдельно, но MVP — без).
pub async fn get_details(rawg_id: u32, api_key: &str) -> Result<RawgGameDetail, RawgError> {
    get_details_with_base(rawg_id, api_key, RAWG_BASE_URL).await
}

async fn get_details_with_base(
    rawg_id: u32,
    api_key: &str,
    base_url: &str,
) -> Result<RawgGameDetail, RawgError> {
    if api_key.is_empty() {
        return Err(RawgError::NoApiKey);
    }
    let url = format!(
        "{}/games/{}?key={}",
        base_url,
        rawg_id,
        urlencoding::encode(api_key),
    );
    let client = build_client()?;
    let resp = client.get(&url).send().await?;
    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        return Err(RawgError::Status(status.as_u16(), text));
    }
    let parsed: RawgGameDetail = serde_json::from_str(&text)?;
    Ok(parsed)
}

// ---------- ArkHost abstraction (для тестируемости) ----------

/// Минимальный trait для ARK операций нужных RAWG apply.
///
/// Реальный `ArkHost` импл этот trait через blanket в конце файла. Тесты —
/// собственный fake, который держит in-memory map of objects.
#[async_trait::async_trait]
pub trait ArkRequester: Send + Sync {
    async fn request(
        &self,
        operation: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, String>;
}

// ---------- Apply metadata ----------

/// Apply RAWG metadata к существующему `game_obj`.
///
/// Этот хелпер использует абстрактный `ArkRequester` чтобы можно было
/// мокать в тестах. Через WS-диспатч мы вызовем `apply_to_game_obj_via_host`
/// с реальным `ArkHost`.
pub async fn apply_to_game_obj<R: ArkRequester>(
    ark: &R,
    game_id: &str,
    rawg_id: u32,
    api_key: &str,
) -> Result<(), RawgError> {
    apply_to_game_obj_with_base(ark, game_id, rawg_id, api_key, RAWG_BASE_URL).await
}

async fn apply_to_game_obj_with_base<R: ArkRequester>(
    ark: &R,
    game_id: &str,
    rawg_id: u32,
    api_key: &str,
    base_url: &str,
) -> Result<(), RawgError> {
    let details = get_details_with_base(rawg_id, api_key, base_url).await?;

    // Fetch existing.
    let existing = ark
        .request("get_object", serde_json::json!({ "id": game_id }))
        .await
        .map_err(RawgError::ArkHost)?;

    // Берём props и merge'им. Если объекта нет — error.
    let props_existing = existing
        .get("propsJson")
        .cloned()
        .unwrap_or(serde_json::json!({}));
    let title_existing = existing
        .get("title")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_default();
    let type_id_existing = existing
        .get("typeId")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "game_obj".to_string());
    if type_id_existing != "game_obj" {
        return Err(RawgError::ArkHost(format!(
            "RAWG metadata can only be applied to game_obj, got {type_id_existing}"
        )));
    }
    let content_existing = existing
        .get("contentJson")
        .cloned()
        .unwrap_or(serde_json::json!({}));

    let mut props = match props_existing {
        serde_json::Value::Object(m) => m,
        _ => serde_json::Map::new(),
    };

    if let Some(img) = details.background_image.clone() {
        props.insert("cover_image".into(), serde_json::Value::String(img.clone()));
        props.insert("background_image".into(), serde_json::Value::String(img));
    }
    if let Some(desc) = details
        .description_raw
        .clone()
        .or_else(|| details.description.clone())
    {
        props.insert("description".into(), serde_json::Value::String(desc));
    }
    if let Some(released) = details.released.clone() {
        props.insert("released".into(), serde_json::Value::String(released));
    }
    let genres: Vec<serde_json::Value> = details
        .genres
        .iter()
        .map(|g| serde_json::Value::String(g.name.clone()))
        .collect();
    props.insert("genres".into(), serde_json::Value::Array(genres));
    let platforms: Vec<serde_json::Value> = details
        .platforms
        .iter()
        .map(|p| serde_json::Value::String(p.platform.name.clone()))
        .collect();
    props.insert("platforms".into(), serde_json::Value::Array(platforms));
    props.insert(
        "rawg_id".into(),
        serde_json::Value::Number(serde_json::Number::from(rawg_id)),
    );

    let now = chrono::Utc::now().to_rfc3339();
    let upsert_payload = serde_json::json!({
        "object": {
            "id": game_id,
            "typeId": type_id_existing,
            "title": title_existing,
            "contentJson": content_existing,
            "propsJson": serde_json::Value::Object(props),
            "createdAt": existing
                .get("createdAt")
                .and_then(|v| v.as_str())
                .unwrap_or(&now),
            "updatedAt": now,
            "deletedAt": existing.get("deletedAt").cloned().unwrap_or(serde_json::Value::Null),
        }
    });

    ark.request("upsert_object", upsert_payload)
        .await
        .map_err(RawgError::ArkHost)?;
    Ok(())
}

// ---------- Implementation: real ArkHost via blanket impl ----------

use crate::ark_host::ArkHost;

#[async_trait::async_trait]
impl ArkRequester for ArkHost {
    async fn request(
        &self,
        operation: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let resp = ArkHost::request(self, operation, params)
            .await
            .map_err(|e| e.to_string())?;
        if !resp.ok {
            return Err(resp.error.unwrap_or_else(|| "ark_host: unknown".into()));
        }
        Ok(resp.data)
    }
}

// ---------- Tests ----------

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;
    use std::sync::Mutex;

    // ---- Fake ArkRequester ----

    struct FakeArk {
        // Singleton object keyed by id. Storage of {id -> json}.
        objects: Mutex<std::collections::HashMap<String, serde_json::Value>>,
        // Captured upserts (latest payload per call).
        upserts: Mutex<Vec<serde_json::Value>>,
    }

    impl FakeArk {
        fn new() -> Self {
            Self {
                objects: Mutex::new(std::collections::HashMap::new()),
                upserts: Mutex::new(Vec::new()),
            }
        }
        fn insert(&self, id: &str, obj: serde_json::Value) {
            self.objects.lock().unwrap().insert(id.to_string(), obj);
        }
    }

    #[async_trait::async_trait]
    impl ArkRequester for FakeArk {
        async fn request(
            &self,
            operation: &str,
            params: serde_json::Value,
        ) -> Result<serde_json::Value, String> {
            match operation {
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
                    if let Some(id) = obj.get("id").and_then(|v| v.as_str()) {
                        self.objects
                            .lock()
                            .unwrap()
                            .insert(id.to_string(), obj.clone());
                    }
                    self.upserts.lock().unwrap().push(obj);
                    Ok(serde_json::json!({ "ok": true }))
                }
                other => Err(format!("unknown op {other}")),
            }
        }
    }

    fn detail_fixture() -> serde_json::Value {
        serde_json::json!({
            "id": 570,
            "name": "Dota 2",
            "description_raw": "MOBA classic.",
            "released": "2013-07-09",
            "background_image": "https://media.rawg.io/dota2.jpg",
            "genres": [{"name": "MOBA"}, {"name": "Strategy"}],
            "platforms": [
                {"platform": {"name": "PC"}},
                {"platform": {"name": "Linux"}}
            ],
            "screenshots": []
        })
    }

    fn search_fixture() -> serde_json::Value {
        serde_json::json!({
            "count": 1,
            "results": [
                {
                    "id": 570,
                    "name": "Dota 2",
                    "slug": "dota-2",
                    "released": "2013-07-09",
                    "background_image": "https://media.rawg.io/dota2.jpg",
                    "rating": 4.21,
                    "genres": [{"name": "Strategy"}],
                    "platforms": [{"platform": {"name": "PC"}}]
                }
            ]
        })
    }

    #[tokio::test]
    async fn rawg_search_serializes_query_params() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(GET)
                    .path("/games")
                    .query_param("search", "dota 2")
                    .query_param("key", "test-key")
                    .query_param("page_size", "20");
                then.status(200)
                    .header("content-type", "application/json")
                    .body(search_fixture().to_string());
            })
            .await;

        let base = server.base_url();
        let results = search_with_base("dota 2", "test-key", &base).await.unwrap();
        mock.assert_async().await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Dota 2");
    }

    #[tokio::test]
    async fn rawg_search_parses_response() {
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(GET).path("/games");
                then.status(200)
                    .header("content-type", "application/json")
                    .body(search_fixture().to_string());
            })
            .await;
        let results = search_with_base("dota", "k", &server.base_url())
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        let first = &results[0];
        assert_eq!(first.name, "Dota 2");
        assert_eq!(first.id, 570);
        assert_eq!(first.slug, "dota-2");
        assert_eq!(first.released.as_deref(), Some("2013-07-09"));
        assert_eq!(first.genres.len(), 1);
        assert_eq!(first.platforms[0].platform.name, "PC");
    }

    #[tokio::test]
    async fn rawg_no_api_key_returns_error() {
        let err = search_with_base("anything", "", "http://example.invalid")
            .await
            .unwrap_err();
        assert!(matches!(err, RawgError::NoApiKey));

        let err2 = get_details_with_base(1, "", "http://example.invalid")
            .await
            .unwrap_err();
        assert!(matches!(err2, RawgError::NoApiKey));
    }

    #[tokio::test]
    async fn rawg_search_propagates_http_error_status() {
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(GET).path("/games");
                then.status(401).body("unauthorized");
            })
            .await;
        let err = search_with_base("x", "bad-key", &server.base_url())
            .await
            .unwrap_err();
        assert!(matches!(err, RawgError::Status(401, _)));
    }

    #[tokio::test]
    async fn rawg_get_details_parses_response() {
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(GET).path("/games/570");
                then.status(200)
                    .header("content-type", "application/json")
                    .body(detail_fixture().to_string());
            })
            .await;
        let detail = get_details_with_base(570, "k", &server.base_url())
            .await
            .unwrap();
        assert_eq!(detail.id, 570);
        assert_eq!(detail.name, "Dota 2");
        assert_eq!(detail.genres.len(), 2);
        assert_eq!(detail.platforms.len(), 2);
        assert_eq!(detail.description_raw.as_deref(), Some("MOBA classic."));
    }

    #[tokio::test]
    async fn rawg_apply_merges_props() {
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(GET).path("/games/570");
                then.status(200)
                    .header("content-type", "application/json")
                    .body(detail_fixture().to_string());
            })
            .await;

        let ark = FakeArk::new();
        ark.insert(
            "game-uuid-1",
            serde_json::json!({
                "id": "game-uuid-1",
                "typeId": "game_obj",
                "title": "Dota 2",
                "contentJson": {},
                "propsJson": {
                    "source": "steam",
                    "source_app_id": "570",
                    "exe_path": "C:/Games/dota.exe"
                },
                "createdAt": "2026-05-01T00:00:00Z",
                "updatedAt": "2026-05-01T00:00:00Z",
                "deletedAt": null
            }),
        );

        apply_to_game_obj_with_base(&ark, "game-uuid-1", 570, "k", &server.base_url())
            .await
            .unwrap();

        let upserts = ark.upserts.lock().unwrap();
        assert_eq!(upserts.len(), 1);
        let upserted = &upserts[0];
        let props = upserted.get("propsJson").unwrap();

        // Original fields preserved.
        assert_eq!(props.get("source").and_then(|v| v.as_str()), Some("steam"));
        assert_eq!(
            props.get("exe_path").and_then(|v| v.as_str()),
            Some("C:/Games/dota.exe")
        );

        // New RAWG fields applied.
        assert_eq!(
            props.get("cover_image").and_then(|v| v.as_str()),
            Some("https://media.rawg.io/dota2.jpg")
        );
        assert_eq!(
            props.get("description").and_then(|v| v.as_str()),
            Some("MOBA classic.")
        );
        assert_eq!(
            props.get("released").and_then(|v| v.as_str()),
            Some("2013-07-09")
        );
        assert_eq!(props.get("rawg_id").and_then(|v| v.as_u64()), Some(570));
        let genres = props.get("genres").and_then(|v| v.as_array()).unwrap();
        assert_eq!(genres.len(), 2);
        assert_eq!(genres[0].as_str(), Some("MOBA"));
        let platforms = props.get("platforms").and_then(|v| v.as_array()).unwrap();
        assert_eq!(platforms.len(), 2);

        // updatedAt bumped (not original).
        assert_ne!(
            upserted.get("updatedAt").and_then(|v| v.as_str()),
            Some("2026-05-01T00:00:00Z")
        );
        // createdAt preserved.
        assert_eq!(
            upserted.get("createdAt").and_then(|v| v.as_str()),
            Some("2026-05-01T00:00:00Z")
        );
    }

    #[tokio::test]
    async fn rawg_apply_errors_when_object_missing() {
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(GET).path("/games/570");
                then.status(200)
                    .header("content-type", "application/json")
                    .body(detail_fixture().to_string());
            })
            .await;

        let ark = FakeArk::new();
        // No object inserted → get_object will fail.
        let err = apply_to_game_obj_with_base(&ark, "missing-id", 570, "k", &server.base_url())
            .await
            .unwrap_err();
        assert!(matches!(err, RawgError::ArkHost(_)));
    }
}
