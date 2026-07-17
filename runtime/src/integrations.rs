use crate::ark_host::ArkHost;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

const CONFIG_FILE: &str = "integrations.json";
const KEYRING_SERVICE: &str = "kosmos-kepler";
const WORKOUT_TYPE_ID: &str = "workout_obj";
const TIME_ENTRY_TYPE_ID: &str = "time_entry_obj";
const CODING_SUBMISSION_TYPE_ID: &str = "coding_submission_obj";
const HEVY_BASE_URL: &str = "https://api.hevyapp.com";
const TOGGL_BASE_URL: &str = "https://api.track.toggl.com/api/v9";
const LEETCODE_GRAPHQL_URL: &str = "https://leetcode.com/graphql";
const ALLOWED_INTERVALS: &[u64] = &[0, 15, 60, 360, 1440];

static CONFIG_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
static SYNC_LOCKS: OnceLock<[tokio::sync::Mutex<()>; 3]> = OnceLock::new();

fn config_lock() -> &'static Mutex<()> {
    CONFIG_LOCK.get_or_init(|| Mutex::new(()))
}

fn sync_lock(index: usize) -> &'static tokio::sync::Mutex<()> {
    &SYNC_LOCKS.get_or_init(|| std::array::from_fn(|_| tokio::sync::Mutex::new(())))[index]
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Hevy,
    Toggl,
    Leetcode,
}

impl Provider {
    fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "hevy" => Ok(Self::Hevy),
            "toggl" => Ok(Self::Toggl),
            "leetcode" => Ok(Self::Leetcode),
            _ => Err(format!("Неизвестная интеграция: {value}")),
        }
    }

    fn id(self) -> &'static str {
        match self {
            Self::Hevy => "hevy",
            Self::Toggl => "toggl",
            Self::Leetcode => "leetcode",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Hevy => "Hevy",
            Self::Toggl => "Toggl Track",
            Self::Leetcode => "LeetCode",
        }
    }

    fn credential_label(self) -> &'static str {
        match self {
            Self::Hevy => "API-ключ",
            Self::Toggl => "API-токен",
            Self::Leetcode => "Сессия LeetCode",
        }
    }

    fn credential_url(self) -> &'static str {
        match self {
            Self::Hevy => "https://hevy.com/settings?developer",
            Self::Toggl => "https://track.toggl.com/profile",
            Self::Leetcode => "https://leetcode.com/accounts/login/",
        }
    }

    fn keyring_user(self) -> &'static str {
        match self {
            Self::Hevy => "integration-hevy-api-key",
            Self::Toggl => "integration-toggl-api-token",
            Self::Leetcode => "integration-leetcode-session",
        }
    }

    fn sync_lock_index(self) -> usize {
        match self {
            Self::Hevy => 0,
            Self::Toggl => 1,
            Self::Leetcode => 2,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProviderSettings {
    pub interval_minutes: u64,
    pub sync_on_startup: bool,
    pub last_attempt_at: Option<String>,
    pub last_success_at: Option<String>,
    pub last_error: Option<String>,
    pub imported_count: u64,
}

impl Default for ProviderSettings {
    fn default() -> Self {
        Self {
            interval_minutes: 0,
            sync_on_startup: false,
            last_attempt_at: None,
            last_success_at: None,
            last_error: None,
            imported_count: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct IntegrationsConfig {
    pub hevy: ProviderSettings,
    pub toggl: ProviderSettings,
    pub leetcode: ProviderSettings,
    pub body_weight_kg: Option<f64>,
}

impl Default for IntegrationsConfig {
    fn default() -> Self {
        Self {
            hevy: ProviderSettings {
                interval_minutes: 360,
                sync_on_startup: true,
                ..ProviderSettings::default()
            },
            toggl: ProviderSettings {
                interval_minutes: 60,
                sync_on_startup: false,
                ..ProviderSettings::default()
            },
            leetcode: ProviderSettings {
                interval_minutes: 1440,
                sync_on_startup: true,
                ..ProviderSettings::default()
            },
            body_weight_kg: None,
        }
    }
}

impl IntegrationsConfig {
    fn provider(&self, provider: Provider) -> &ProviderSettings {
        match provider {
            Provider::Hevy => &self.hevy,
            Provider::Toggl => &self.toggl,
            Provider::Leetcode => &self.leetcode,
        }
    }

    fn provider_mut(&mut self, provider: Provider) -> &mut ProviderSettings {
        match provider {
            Provider::Hevy => &mut self.hevy,
            Provider::Toggl => &mut self.toggl,
            Provider::Leetcode => &mut self.leetcode,
        }
    }
}

fn config_path(data_dir: &Path) -> PathBuf {
    data_dir.join(CONFIG_FILE)
}

fn read_config_unlocked(data_dir: &Path) -> IntegrationsConfig {
    fs::read_to_string(config_path(data_dir))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn read_config(data_dir: &Path) -> IntegrationsConfig {
    let _guard = config_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    read_config_unlocked(data_dir)
}

fn write_config_unlocked(data_dir: &Path, config: &IntegrationsConfig) -> Result<(), String> {
    fs::create_dir_all(data_dir).map_err(|error| format!("Не удалось создать папку: {error}"))?;
    let target = config_path(data_dir);
    let temporary = target.with_extension("json.tmp");
    let json = serde_json::to_vec_pretty(config)
        .map_err(|error| format!("Не удалось сериализовать настройки: {error}"))?;
    fs::write(&temporary, json)
        .map_err(|error| format!("Не удалось сохранить настройки: {error}"))?;
    fs::rename(&temporary, &target)
        .map_err(|error| format!("Не удалось применить настройки: {error}"))
}

fn mutate_config(
    data_dir: &Path,
    mutate: impl FnOnce(&mut IntegrationsConfig) -> Result<(), String>,
) -> Result<IntegrationsConfig, String> {
    let _guard = config_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut config = read_config_unlocked(data_dir);
    mutate(&mut config)?;
    write_config_unlocked(data_dir, &config)?;
    Ok(config)
}

#[cfg(not(test))]
fn credential_entry(provider: Provider) -> Result<keyring::Entry, String> {
    let service = if std::env::var("KOSMOS_TEST_MODE").as_deref() == Ok("1") {
        "kosmos-kepler-test"
    } else {
        KEYRING_SERVICE
    };
    keyring::Entry::new(service, provider.keyring_user())
        .map_err(|error| format!("Windows Credential Manager: {error}"))
}

#[cfg(not(test))]
fn read_credential(provider: Provider) -> Option<String> {
    credential_entry(provider).ok()?.get_password().ok()
}

#[cfg(test)]
fn read_credential(_provider: Provider) -> Option<String> {
    None
}

#[cfg(not(test))]
fn save_credential(provider: Provider, secret: &str) -> Result<(), String> {
    credential_entry(provider)?
        .set_password(secret)
        .map_err(|error| format!("Не удалось сохранить ключ: {error}"))
}

#[cfg(test)]
fn save_credential(_provider: Provider, _secret: &str) -> Result<(), String> {
    Ok(())
}

#[cfg(not(test))]
fn delete_credential(provider: Provider) -> Result<(), String> {
    match credential_entry(provider)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(format!("Не удалось удалить ключ: {error}")),
    }
}

#[cfg(test)]
fn delete_credential(_provider: Provider) -> Result<(), String> {
    Ok(())
}

fn provider_snapshot(provider: Provider, settings: &ProviderSettings) -> Value {
    json!({
        "id": provider.id(),
        "label": provider.label(),
        "credentialLabel": provider.credential_label(),
        "credentialUrl": provider.credential_url(),
        "hasCredential": read_credential(provider).is_some(),
        "settings": settings,
    })
}

fn snapshot(config: &IntegrationsConfig) -> Value {
    json!({
        "providers": [
            provider_snapshot(Provider::Hevy, &config.hevy),
            provider_snapshot(Provider::Toggl, &config.toggl),
            provider_snapshot(Provider::Leetcode, &config.leetcode),
        ],
        "bodyWeightKg": config.body_weight_kg,
    })
}

fn provider_from_params(params: &Value) -> Result<Provider, String> {
    Provider::parse(
        params
            .get("provider")
            .and_then(Value::as_str)
            .ok_or("Не указана интеграция")?,
    )
}

fn validate_interval(value: u64) -> Result<u64, String> {
    ALLOWED_INTERVALS
        .contains(&value)
        .then_some(value)
        .ok_or_else(|| "Недопустимая частота синхронизации".to_string())
}

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("Kosmos/1 integrations")
        .build()
        .map_err(|error| format!("Не удалось создать HTTP-клиент: {error}"))
}

fn authenticated_get(
    client: &reqwest::Client,
    provider: Provider,
    url: String,
    secret: &str,
) -> reqwest::RequestBuilder {
    match provider {
        Provider::Hevy => client.get(url).header("api-key", secret),
        Provider::Toggl => client.get(url).basic_auth(secret, Some("api_token")),
        Provider::Leetcode => client.get(url),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LeetcodeCredential {
    session: String,
    csrf_token: String,
}

fn leetcode_request(
    client: &reqwest::Client,
    secret: &str,
    body: Value,
) -> Result<reqwest::RequestBuilder, String> {
    let credential: LeetcodeCredential = serde_json::from_str(secret)
        .map_err(|_| "Сессия LeetCode повреждена — войдите заново".to_string())?;
    Ok(client
        .post(LEETCODE_GRAPHQL_URL)
        .header(
            reqwest::header::COOKIE,
            format!(
                "LEETCODE_SESSION={}; csrftoken={}",
                credential.session, credential.csrf_token
            ),
        )
        .header("x-csrftoken", credential.csrf_token)
        .header(reqwest::header::ORIGIN, "https://leetcode.com")
        .header(reqwest::header::REFERER, "https://leetcode.com/progress/")
        .json(&body))
}

fn leetcode_query(offset: u64, last_key: Option<&str>) -> Value {
    json!({
        "query": "query submissionList($offset: Int!, $limit: Int!, $lastKey: String) { submissionList(offset: $offset, limit: $limit, lastKey: $lastKey) { lastKey hasNext submissions { id title titleSlug statusDisplay lang timestamp url isPending memory runtime } } }",
        "variables": {
            "offset": offset,
            "limit": 20,
            "lastKey": last_key,
        }
    })
}

async fn verify_credential_at(
    provider: Provider,
    secret: &str,
    base_url: &str,
) -> Result<(), String> {
    let client = http_client()?;
    let path = match provider {
        Provider::Hevy => "/v1/user/info",
        Provider::Toggl => "/me",
        Provider::Leetcode => {
            let response = leetcode_request(&client, secret, leetcode_query(0, None))?
                .send()
                .await
                .map_err(|error| format!("Не удалось подключиться к LeetCode: {error}"))?;
            if response.status() == reqwest::StatusCode::UNAUTHORIZED
                || response.status() == reqwest::StatusCode::FORBIDDEN
            {
                return Err("Сессия LeetCode истекла — войдите заново".to_string());
            }
            let body: Value = response
                .json()
                .await
                .map_err(|error| format!("Некорректный ответ LeetCode: {error}"))?;
            return body
                .pointer("/data/submissionList/submissions")
                .and_then(Value::as_array)
                .map(|_| ())
                .ok_or_else(|| "LeetCode не вернул историю — войдите заново".to_string());
        }
    };
    let request = authenticated_get(&client, provider, format!("{base_url}{path}"), secret);
    let response = request
        .send()
        .await
        .map_err(|error| format!("Не удалось подключиться к {}: {error}", provider.label()))?;
    if response.status().is_success() {
        Ok(())
    } else if response.status() == reqwest::StatusCode::UNAUTHORIZED
        || response.status() == reqwest::StatusCode::FORBIDDEN
    {
        Err("Ключ не принят сервисом".to_string())
    } else {
        Err(format!(
            "{} вернул HTTP {}",
            provider.label(),
            response.status()
        ))
    }
}

async fn verify_credential(provider: Provider, secret: &str) -> Result<(), String> {
    verify_credential_at(
        provider,
        secret,
        match provider {
            Provider::Hevy => HEVY_BASE_URL,
            Provider::Toggl => TOGGL_BASE_URL,
            Provider::Leetcode => LEETCODE_GRAPHQL_URL,
        },
    )
    .await
}

async fn ark_request(ark: &ArkHost, operation: &str, params: Value) -> Result<Value, String> {
    let response = ark
        .request(operation, params)
        .await
        .map_err(|error| format!("ARK {operation}: {error}"))?;
    if response.ok {
        Ok(response.data)
    } else {
        Err(format!(
            "ARK {operation}: {}",
            response
                .error
                .unwrap_or_else(|| "unknown error".to_string())
        ))
    }
}

async fn ensure_object_type(ark: &ArkHost, id: &str, name: &str, now: &str) -> Result<(), String> {
    ark_request(
        ark,
        "upsert_object_type",
        json!({
            "object_type": {
                "id": id,
                "name": name,
                "schemaJson": "{}",
                "uiSchemaJson": "{}",
                "createdAt": now,
                "updatedAt": now,
                "systemLocked": false,
            }
        }),
    )
    .await?;
    Ok(())
}

async fn fetch_hevy_pages(
    client: &reqwest::Client,
    secret: &str,
    path: &str,
    item_key: &str,
    page_size: u32,
    extra_query: &[(&str, String)],
) -> Result<Vec<Value>, String> {
    let mut page = 1_u32;
    let mut items = Vec::new();
    loop {
        let mut request = authenticated_get(
            client,
            Provider::Hevy,
            format!("{HEVY_BASE_URL}{path}"),
            secret,
        )
        .query(&[
            ("page", page.to_string()),
            ("pageSize", page_size.to_string()),
        ]);
        if !extra_query.is_empty() {
            request = request.query(extra_query);
        }
        let response = request
            .send()
            .await
            .map_err(|error| format!("Hevy: {error}"))?;
        if !response.status().is_success() {
            return Err(format!("Hevy вернул HTTP {}", response.status()));
        }
        let body: Value = response
            .json()
            .await
            .map_err(|error| format!("Некорректный ответ Hevy: {error}"))?;
        items.extend(
            body.get(item_key)
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default(),
        );
        let page_count = body.get("page_count").and_then(Value::as_u64).unwrap_or(1);
        if u64::from(page) >= page_count || page >= 10_000 {
            break;
        }
        page += 1;
    }
    Ok(items)
}

fn annotate_hevy_workout(workout: &mut Value, templates: &HashMap<String, Value>) {
    let Some(exercises) = workout.get_mut("exercises").and_then(Value::as_array_mut) else {
        return;
    };
    for exercise in exercises {
        let Some(object) = exercise.as_object_mut() else {
            continue;
        };
        let Some(template) = object
            .get("exercise_template_id")
            .and_then(Value::as_str)
            .and_then(|id| templates.get(id))
        else {
            continue;
        };
        for (source, target) in [
            ("primary_muscle_group", "primaryMuscleGroup"),
            ("secondary_muscle_groups", "secondaryMuscleGroups"),
            ("equipment_category", "equipmentCategory"),
            ("type", "exerciseType"),
        ] {
            if let Some(value) = template.get(source) {
                object.insert(target.to_string(), value.clone());
            }
        }
    }
}

fn hevy_workout_object(workout: &Value) -> Result<Value, String> {
    let external_id = workout
        .get("id")
        .and_then(Value::as_str)
        .ok_or("Hevy workout без id")?;
    let title = workout
        .get("title")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("Тренировка");
    let created_at = workout
        .get("created_at")
        .or_else(|| workout.get("start_time"))
        .and_then(Value::as_str)
        .unwrap_or_else(|| "1970-01-01T00:00:00Z");
    let updated_at = workout
        .get("updated_at")
        .and_then(Value::as_str)
        .unwrap_or(created_at);
    Ok(json!({
        "id": format!("hevy-workout:{external_id}"),
        "typeId": WORKOUT_TYPE_ID,
        "title": title,
        "contentJson": { "description": workout.get("description").cloned().unwrap_or(Value::Null) },
        "propsJson": {
            "source": "hevy",
            "externalId": external_id,
            "routineId": workout.get("routine_id").cloned().unwrap_or(Value::Null),
            "startedAt": workout.get("start_time").cloned().unwrap_or(Value::Null),
            "endedAt": workout.get("end_time").cloned().unwrap_or(Value::Null),
            "providerUpdatedAt": workout.get("updated_at").cloned().unwrap_or(Value::Null),
            "exercises": workout.get("exercises").cloned().unwrap_or_else(|| json!([])),
        },
        "createdAt": created_at,
        "updatedAt": updated_at,
        "deletedAt": null,
    }))
}

async fn sync_hevy(
    ark: &ArkHost,
    secret: &str,
    settings: &ProviderSettings,
    started_at: &str,
) -> Result<u64, String> {
    let client = http_client()?;
    let templates = fetch_hevy_pages(
        &client,
        secret,
        "/v1/exercise_templates",
        "exercise_templates",
        100,
        &[],
    )
    .await?
    .into_iter()
    .filter_map(|template| {
        let id = template.get("id")?.as_str()?.to_string();
        Some((id, template))
    })
    .collect::<HashMap<_, _>>();

    let mut deleted_ids = Vec::new();
    let workouts = if let Some(since) = settings.last_success_at.as_ref() {
        let events = fetch_hevy_pages(
            &client,
            secret,
            "/v1/workouts/events",
            "events",
            10,
            &[("since", since.clone())],
        )
        .await?;
        let mut updated = Vec::new();
        for event in events {
            match event.get("type").and_then(Value::as_str) {
                Some("updated") => {
                    if let Some(workout) = event.get("workout") {
                        updated.push(workout.clone());
                    }
                }
                Some("deleted") => {
                    if let Some(id) = event.get("id").and_then(Value::as_str) {
                        deleted_ids.push(id.to_string());
                    }
                }
                _ => {}
            }
        }
        updated
    } else {
        fetch_hevy_pages(&client, secret, "/v1/workouts", "workouts", 10, &[]).await?
    };

    ensure_object_type(ark, WORKOUT_TYPE_ID, "Тренировка", started_at).await?;
    let mut imported = 0_u64;
    for mut workout in workouts {
        annotate_hevy_workout(&mut workout, &templates);
        ark_request(
            ark,
            "upsert_object",
            json!({ "object": hevy_workout_object(&workout)? }),
        )
        .await?;
        imported += 1;
    }
    for external_id in deleted_ids {
        ark_request(
            ark,
            "delete_object",
            json!({ "id": format!("hevy-workout:{external_id}") }),
        )
        .await?;
    }
    Ok(imported)
}

fn toggl_time_entry_object(entry: &Value) -> Result<Value, String> {
    let external_id = entry
        .get("id")
        .and_then(Value::as_i64)
        .ok_or("Toggl time entry без id")?;
    let started_at = entry
        .get("start")
        .and_then(Value::as_str)
        .ok_or("Toggl time entry без start")?;
    let updated_at = entry
        .get("at")
        .and_then(Value::as_str)
        .unwrap_or(started_at);
    let title = entry
        .get("description")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("Toggl Track");
    Ok(json!({
        "id": format!("toggl-time-entry:{external_id}"),
        "typeId": TIME_ENTRY_TYPE_ID,
        "title": title,
        "contentJson": {},
        "propsJson": {
            "startedAt": started_at,
            "endedAt": entry.get("stop").cloned().unwrap_or(Value::Null),
            "durationSeconds": entry.get("duration").cloned().unwrap_or(Value::Null),
            "billable": entry.get("billable").cloned().unwrap_or(Value::Bool(false)),
            "projectId": entry.get("project_id").or_else(|| entry.get("pid")).cloned().unwrap_or(Value::Null),
            "workspaceId": entry.get("workspace_id").or_else(|| entry.get("wid")).cloned().unwrap_or(Value::Null),
            "tags": entry.get("tags").cloned().unwrap_or_else(|| json!([])),
            "source": "imported",
            "provider": "toggl",
            "externalId": external_id,
        },
        "createdAt": started_at,
        "updatedAt": updated_at,
        "deletedAt": null,
    }))
}

fn toggl_before_cursor(entries: &[Value]) -> Option<String> {
    entries
        .iter()
        .filter_map(|entry| entry.get("start").and_then(Value::as_str))
        .filter_map(|value| DateTime::parse_from_rfc3339(value).ok())
        .min()
        .map(|value| (value - ChronoDuration::milliseconds(1)).to_rfc3339())
}

async fn fetch_toggl_entries(
    client: &reqwest::Client,
    secret: &str,
    since: Option<i64>,
) -> Result<Vec<Value>, String> {
    let mut entries = Vec::new();
    let mut seen = HashSet::new();
    let mut before: Option<String> = None;

    loop {
        let mut request = authenticated_get(
            client,
            Provider::Toggl,
            format!("{TOGGL_BASE_URL}/me/time_entries"),
            secret,
        );
        if let Some(value) = since {
            request = request.query(&[("since", value.to_string())]);
        }
        if let Some(value) = before.as_ref() {
            request = request.query(&[("before", value)]);
        }

        let response = request
            .send()
            .await
            .map_err(|error| format!("Toggl Track: {error}"))?;
        if !response.status().is_success() {
            return Err(format!("Toggl Track вернул HTTP {}", response.status()));
        }
        let page: Vec<Value> = response
            .json()
            .await
            .map_err(|error| format!("Некорректный ответ Toggl Track: {error}"))?;
        if page.is_empty() {
            break;
        }

        let Some(next_before) = toggl_before_cursor(&page) else {
            break;
        };
        for entry in page {
            if entry
                .get("id")
                .and_then(Value::as_i64)
                .is_none_or(|id| seen.insert(id))
            {
                entries.push(entry);
            }
        }
        if before
            .as_deref()
            .is_some_and(|value| value <= next_before.as_str())
        {
            break;
        }
        before = Some(next_before);
    }

    Ok(entries)
}

async fn sync_toggl(
    ark: &ArkHost,
    secret: &str,
    settings: &ProviderSettings,
    started_at: &str,
) -> Result<u64, String> {
    let client = http_client()?;
    let since = settings
        .last_success_at
        .as_deref()
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.timestamp());
    let entries = fetch_toggl_entries(&client, secret, since).await?;

    ensure_object_type(ark, TIME_ENTRY_TYPE_ID, "Запись времени", started_at).await?;
    let mut imported = 0_u64;
    for entry in entries {
        let external_id = entry.get("id").and_then(Value::as_i64);
        let deleted = entry
            .get("server_deleted_at")
            .or_else(|| entry.get("deleted_at"))
            .is_some_and(|value| !value.is_null());
        if deleted {
            if let Some(id) = external_id {
                ark_request(
                    ark,
                    "delete_object",
                    json!({ "id": format!("toggl-time-entry:{id}") }),
                )
                .await?;
            }
            continue;
        }
        ark_request(
            ark,
            "upsert_object",
            json!({ "object": toggl_time_entry_object(&entry)? }),
        )
        .await?;
        imported += 1;
    }
    Ok(imported)
}

fn leetcode_submission_timestamp(submission: &Value) -> Option<DateTime<Utc>> {
    submission
        .get("timestamp")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<i64>().ok())
        .and_then(|value| DateTime::<Utc>::from_timestamp(value, 0))
}

fn leetcode_submission_is_new_enough(submission: &Value, cutoff: Option<DateTime<Utc>>) -> bool {
    cutoff.is_none_or(|cutoff| {
        leetcode_submission_timestamp(submission).is_none_or(|value| value >= cutoff)
    })
}

fn leetcode_page_reached_cutoff(items: &[Value], cutoff: Option<DateTime<Utc>>) -> bool {
    cutoff.is_some_and(|cutoff| {
        items.iter().any(|submission| {
            leetcode_submission_timestamp(submission).is_some_and(|value| value < cutoff)
        })
    })
}

fn leetcode_submission_object(submission: &Value) -> Result<Value, String> {
    let external_id = submission
        .get("id")
        .and_then(Value::as_str)
        .ok_or("LeetCode submission без id")?;
    let title = submission
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("LeetCode");
    let status = submission
        .get("statusDisplay")
        .and_then(Value::as_str)
        .unwrap_or("Unknown");
    let timestamp = leetcode_submission_timestamp(submission)
        .map(|value| value.to_rfc3339())
        .ok_or("LeetCode submission без корректной даты")?;
    let raw_url = submission.get("url").and_then(Value::as_str).unwrap_or("");
    let url = if raw_url.starts_with("http") {
        raw_url.to_string()
    } else {
        format!("https://leetcode.com{raw_url}")
    };
    Ok(json!({
        "id": format!("leetcode-submission:{external_id}"),
        "typeId": CODING_SUBMISSION_TYPE_ID,
        "title": format!("{title} — {status}"),
        "contentJson": {},
        "propsJson": {
            "source": "leetcode",
            "externalId": external_id,
            "problemTitle": title,
            "problemSlug": submission.get("titleSlug").cloned().unwrap_or(Value::Null),
            "status": status,
            "accepted": status == "Accepted",
            "language": submission.get("lang").cloned().unwrap_or(Value::Null),
            "runtime": submission.get("runtime").cloned().unwrap_or(Value::Null),
            "memory": submission.get("memory").cloned().unwrap_or(Value::Null),
            "submittedAt": timestamp,
            "url": url,
        },
        "createdAt": timestamp,
        "updatedAt": timestamp,
        "deletedAt": null,
    }))
}

async fn fetch_leetcode_submissions(
    client: &reqwest::Client,
    secret: &str,
    cutoff: Option<DateTime<Utc>>,
) -> Result<Vec<Value>, String> {
    let mut submissions = Vec::new();
    let mut seen = HashSet::new();
    let mut offset = 0_u64;
    let mut last_key: Option<String> = None;
    loop {
        let response =
            leetcode_request(client, secret, leetcode_query(offset, last_key.as_deref()))?
                .send()
                .await
                .map_err(|error| format!("LeetCode: {error}"))?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED
            || response.status() == reqwest::StatusCode::FORBIDDEN
        {
            return Err("Сессия LeetCode истекла — войдите заново".to_string());
        }
        if !response.status().is_success() {
            return Err(format!("LeetCode вернул HTTP {}", response.status()));
        }
        let body: Value = response
            .json()
            .await
            .map_err(|error| format!("Некорректный ответ LeetCode: {error}"))?;
        if let Some(error) = body
            .get("errors")
            .and_then(Value::as_array)
            .and_then(|errors| errors.first())
            .and_then(|error| error.get("message"))
            .and_then(Value::as_str)
        {
            return Err(format!("LeetCode: {error}"));
        }
        let page = body
            .pointer("/data/submissionList")
            .ok_or("LeetCode не вернул историю — войдите заново")?;
        let items = page
            .get("submissions")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let page_len = items.len() as u64;
        let reached_cutoff = leetcode_page_reached_cutoff(&items, cutoff);
        for submission in items {
            if !leetcode_submission_is_new_enough(&submission, cutoff) {
                continue;
            }
            if submission
                .get("id")
                .and_then(Value::as_str)
                .is_some_and(|id| seen.insert(id.to_string()))
            {
                submissions.push(submission);
            }
        }
        if !page
            .get("hasNext")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            || reached_cutoff
            || page_len == 0
            || offset >= 10_000
        {
            break;
        }
        offset += page_len;
        last_key = page
            .get("lastKey")
            .and_then(Value::as_str)
            .map(str::to_string);
    }
    Ok(submissions)
}

async fn sync_leetcode(
    ark: &ArkHost,
    secret: &str,
    settings: &ProviderSettings,
    started_at: &str,
) -> Result<u64, String> {
    let client = http_client()?;
    let cutoff = settings
        .last_success_at
        .as_deref()
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| value.with_timezone(&Utc) - ChronoDuration::days(1));
    let submissions = fetch_leetcode_submissions(&client, secret, cutoff).await?;
    ensure_object_type(
        ark,
        CODING_SUBMISSION_TYPE_ID,
        "Отправка задачи",
        started_at,
    )
    .await?;
    let mut imported = 0_u64;
    for submission in submissions {
        ark_request(
            ark,
            "upsert_object",
            json!({ "object": leetcode_submission_object(&submission)? }),
        )
        .await?;
        imported += 1;
    }
    Ok(imported)
}

async fn sync_provider(
    ark: &ArkHost,
    data_dir: &Path,
    provider: Provider,
) -> Result<Value, String> {
    let _guard = sync_lock(provider.sync_lock_index())
        .try_lock()
        .map_err(|_| format!("{} уже синхронизируется", provider.label()))?;
    let secret = read_credential(provider).ok_or_else(|| "Сначала добавьте ключ".to_string())?;
    let started_at = Utc::now().to_rfc3339();
    let settings = mutate_config(data_dir, |config| {
        let settings = config.provider_mut(provider);
        settings.last_attempt_at = Some(started_at.clone());
        settings.last_error = None;
        Ok(())
    })?
    .provider(provider)
    .clone();

    let result = match provider {
        Provider::Hevy => sync_hevy(ark, &secret, &settings, &started_at).await,
        Provider::Toggl => sync_toggl(ark, &secret, &settings, &started_at).await,
        Provider::Leetcode => sync_leetcode(ark, &secret, &settings, &started_at).await,
    };
    match result {
        Ok(imported) => {
            let config = mutate_config(data_dir, |config| {
                let settings = config.provider_mut(provider);
                settings.last_success_at = Some(started_at.clone());
                settings.last_error = None;
                settings.imported_count = imported;
                Ok(())
            })?;
            Ok(json!({ "imported": imported, "snapshot": snapshot(&config) }))
        }
        Err(error) => {
            let message = error.to_string();
            let _ = mutate_config(data_dir, |config| {
                config.provider_mut(provider).last_error = Some(message.clone());
                Ok(())
            });
            Err(message)
        }
    }
}

fn is_due(settings: &ProviderSettings, now: DateTime<Utc>) -> bool {
    if settings.interval_minutes == 0 {
        return false;
    }
    settings
        .last_attempt_at
        .as_deref()
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|last| {
            now - last.with_timezone(&Utc)
                >= ChronoDuration::minutes(settings.interval_minutes as i64)
        })
        .unwrap_or(true)
}

pub fn spawn_scheduler(ark: Arc<ArkHost>, data_dir: PathBuf) {
    tokio::spawn(async move {
        let startup = read_config(&data_dir);
        for provider in [Provider::Hevy, Provider::Toggl, Provider::Leetcode] {
            if startup.provider(provider).sync_on_startup && read_credential(provider).is_some() {
                if let Err(error) = sync_provider(&ark, &data_dir, provider).await {
                    tracing::warn!(provider = provider.id(), %error, "integration startup sync failed");
                }
            }
        }

        let mut timer = tokio::time::interval(Duration::from_secs(60));
        timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        timer.tick().await;
        loop {
            timer.tick().await;
            let config = read_config(&data_dir);
            let now = Utc::now();
            for provider in [Provider::Hevy, Provider::Toggl, Provider::Leetcode] {
                if read_credential(provider).is_some() && is_due(config.provider(provider), now) {
                    if let Err(error) = sync_provider(&ark, &data_dir, provider).await {
                        tracing::warn!(provider = provider.id(), %error, "integration scheduled sync failed");
                    }
                }
            }
        }
    });
}

fn muscle_slug(value: &str) -> Option<&'static str> {
    match value
        .trim()
        .to_ascii_lowercase()
        .replace([' ', '-'], "_")
        .as_str()
    {
        "chest" => Some("chest"),
        "shoulders" | "front_deltoids" => Some("front-deltoids"),
        "rear_shoulders" | "back_deltoids" => Some("back-deltoids"),
        "biceps" => Some("biceps"),
        "triceps" => Some("triceps"),
        "forearms" | "forearm" => Some("forearm"),
        "upper_back" | "lats" => Some("upper-back"),
        "lower_back" => Some("lower-back"),
        "traps" | "trapezius" => Some("trapezius"),
        "abs" | "abdominals" => Some("abs"),
        "obliques" => Some("obliques"),
        "quadriceps" | "quads" => Some("quadriceps"),
        "hamstrings" | "hamstring" => Some("hamstring"),
        "glutes" | "gluteal" => Some("gluteal"),
        "calves" => Some("calves"),
        "adductors" | "adductor" => Some("adductor"),
        "abductors" | "abductor" => Some("abductors"),
        "neck" => Some("neck"),
        _ => None,
    }
}

fn benchmark_ratio(muscle: &str) -> f64 {
    match muscle {
        "chest" => 1.25,
        "front-deltoids" | "back-deltoids" => 0.75,
        "biceps" | "triceps" | "forearm" => 0.5,
        "upper-back" | "trapezius" => 1.25,
        "lower-back" | "hamstring" | "gluteal" => 1.75,
        "quadriceps" | "calves" => 1.5,
        "abs" | "obliques" | "adductor" | "abductors" | "neck" => 1.0,
        _ => 1.0,
    }
}

fn equipment_is_bodyweight(value: Option<&Value>) -> bool {
    value
        .and_then(Value::as_str)
        .is_some_and(|value| matches!(value, "none" | "bodyweight"))
}

#[derive(Debug, Default, Clone)]
struct DevelopmentMetric {
    score: f64,
    best_e1rm_kg: f64,
    exercise: String,
}

fn body_metrics(objects: &[Value], body_weight_kg: Option<f64>, range_days: i64) -> Value {
    let now = Utc::now();
    let cutoff = now - ChronoDuration::days(range_days);
    let mut development: HashMap<String, DevelopmentMetric> = HashMap::new();
    let mut load: HashMap<String, f64> = HashMap::new();
    let mut workout_count = 0_u64;

    for object in objects {
        let props = object.get("propsJson").unwrap_or(&Value::Null);
        if props.get("source").and_then(Value::as_str) != Some("hevy") {
            continue;
        }
        let started = props
            .get("startedAt")
            .and_then(Value::as_str)
            .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
            .map(|value| value.with_timezone(&Utc));
        if started.is_some_and(|value| value >= cutoff) {
            workout_count += 1;
        }
        let Some(exercises) = props.get("exercises").and_then(Value::as_array) else {
            continue;
        };
        for exercise in exercises {
            let Some(primary) = exercise
                .get("primaryMuscleGroup")
                .and_then(Value::as_str)
                .and_then(muscle_slug)
            else {
                continue;
            };
            let secondary = exercise
                .get("secondaryMuscleGroups")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .filter_map(muscle_slug)
                .collect::<Vec<_>>();
            let bodyweight = equipment_is_bodyweight(exercise.get("equipmentCategory"));
            let exercise_title = exercise
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or("Упражнение");
            let Some(sets) = exercise.get("sets").and_then(Value::as_array) else {
                continue;
            };
            for set in sets {
                if set.get("type").and_then(Value::as_str) == Some("warmup") {
                    continue;
                }
                let reps = set.get("reps").and_then(Value::as_f64).unwrap_or(0.0);
                if reps <= 0.0 {
                    continue;
                }
                let added_weight = set.get("weight_kg").and_then(Value::as_f64).unwrap_or(0.0);
                let effective_weight = if bodyweight {
                    body_weight_kg.unwrap_or(0.0) + added_weight
                } else {
                    added_weight
                };
                if effective_weight <= 0.0 {
                    continue;
                }
                if let Some(weight) = body_weight_kg.filter(|weight| *weight > 0.0) {
                    let e1rm = effective_weight * (1.0 + reps.min(30.0) / 30.0);
                    let score = (e1rm / (weight * benchmark_ratio(primary))).clamp(0.0, 1.0);
                    let metric = development.entry(primary.to_string()).or_default();
                    if score > metric.score {
                        metric.score = score;
                        metric.best_e1rm_kg = e1rm;
                        metric.exercise = exercise_title.to_string();
                    }
                    for muscle in &secondary {
                        let metric = development.entry((*muscle).to_string()).or_default();
                        let secondary_score = score * 0.6;
                        if secondary_score > metric.score {
                            metric.score = secondary_score;
                            metric.best_e1rm_kg = e1rm;
                            metric.exercise = exercise_title.to_string();
                        }
                    }
                }
                if started.is_some_and(|value| value >= cutoff) {
                    let tonnage = effective_weight * reps;
                    *load.entry(primary.to_string()).or_default() += tonnage;
                    for muscle in &secondary {
                        *load.entry((*muscle).to_string()).or_default() += tonnage * 0.5;
                    }
                }
            }
        }
    }

    let max_load = load.values().copied().fold(0.0_f64, f64::max);
    let development = development
        .into_iter()
        .map(|(muscle, metric)| {
            json!({
                "muscle": muscle,
                "score": metric.score,
                "level": (metric.score * 5.0).ceil().clamp(0.0, 5.0) as u8,
                "bestE1rmKg": metric.best_e1rm_kg,
                "exercise": metric.exercise,
            })
        })
        .collect::<Vec<_>>();
    let load = load
        .into_iter()
        .map(|(muscle, tonnage)| {
            json!({
                "muscle": muscle,
                "tonnageKg": tonnage,
                "intensity": if max_load > 0.0 { tonnage / max_load } else { 0.0 },
            })
        })
        .collect::<Vec<_>>();

    json!({
        "bodyWeightKg": body_weight_kg,
        "needsBodyWeight": body_weight_kg.is_none(),
        "workoutCount": workout_count,
        "rangeDays": range_days,
        "development": development,
        "load": load,
    })
}

async fn body_snapshot(ark: &ArkHost, data_dir: &Path, range: &str) -> Result<Value, String> {
    let range_days = match range {
        "week" => 7,
        "month" => 30,
        "year" => 365,
        _ => return Err("Неизвестный период нагрузки".to_string()),
    };
    let objects = ark_request(
        ark,
        "list_objects_by_type",
        json!({ "type_id": WORKOUT_TYPE_ID }),
    )
    .await?
    .as_array()
    .cloned()
    .unwrap_or_default();
    Ok(body_metrics(
        &objects,
        read_config(data_dir).body_weight_kg,
        range_days,
    ))
}

pub async fn handle_operation(
    subop: &str,
    params: Value,
    ark: &ArkHost,
    data_dir: &Path,
) -> Result<Value, String> {
    match subop {
        "list" => Ok(snapshot(&read_config(data_dir))),
        "update_settings" => {
            let provider = provider_from_params(&params)?;
            let interval = params
                .get("intervalMinutes")
                .and_then(Value::as_u64)
                .map(validate_interval)
                .transpose()?;
            let startup = params.get("syncOnStartup").and_then(Value::as_bool);
            let config = mutate_config(data_dir, |config| {
                let settings = config.provider_mut(provider);
                if let Some(value) = interval {
                    settings.interval_minutes = value;
                }
                if let Some(value) = startup {
                    settings.sync_on_startup = value;
                }
                Ok(())
            })?;
            Ok(snapshot(&config))
        }
        "set_credential" => {
            let provider = provider_from_params(&params)?;
            let secret = params
                .get("credential")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or("Введите ключ")?;
            verify_credential(provider, secret).await?;
            save_credential(provider, secret)?;
            Ok(snapshot(&read_config(data_dir)))
        }
        "clear_credential" => {
            delete_credential(provider_from_params(&params)?)?;
            Ok(snapshot(&read_config(data_dir)))
        }
        "sync_now" => sync_provider(ark, data_dir, provider_from_params(&params)?).await,
        "body_weight_set" => {
            let body_weight = params.get("bodyWeightKg").and_then(Value::as_f64);
            if body_weight.is_some_and(|value| !(20.0..=400.0).contains(&value)) {
                return Err("Вес тела должен быть от 20 до 400 кг".to_string());
            }
            let config = mutate_config(data_dir, |config| {
                config.body_weight_kg = body_weight;
                Ok(())
            })?;
            Ok(snapshot(&config))
        }
        "body_snapshot" => {
            body_snapshot(
                ark,
                data_dir,
                params
                    .get("range")
                    .and_then(Value::as_str)
                    .unwrap_or("week"),
            )
            .await
        }
        _ => Err(format!("Неизвестная операция integrations.{subop}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hevy_mapping_and_body_metrics_are_stable() {
        let mut workout = json!({
            "id": "workout-1",
            "title": "Push",
            "start_time": Utc::now().to_rfc3339(),
            "end_time": Utc::now().to_rfc3339(),
            "created_at": Utc::now().to_rfc3339(),
            "updated_at": Utc::now().to_rfc3339(),
            "exercises": [{
                "title": "Bench Press (Barbell)",
                "exercise_template_id": "bench",
                "sets": [{ "type": "normal", "weight_kg": 100.0, "reps": 5.0 }]
            }]
        });
        annotate_hevy_workout(
            &mut workout,
            &HashMap::from([(
                "bench".to_string(),
                json!({
                    "primary_muscle_group": "chest",
                    "secondary_muscle_groups": ["triceps"],
                    "equipment_category": "barbell",
                    "type": "weight_reps"
                }),
            )]),
        );
        let object = hevy_workout_object(&workout).expect("map workout");
        let metrics = body_metrics(&[object], Some(80.0), 7);
        let development = metrics["development"].as_array().expect("development");
        assert!(development.iter().any(|row| row["muscle"] == "chest"));
        assert!(development.iter().any(|row| row["muscle"] == "triceps"));
        let load = metrics["load"].as_array().expect("load");
        let chest = load
            .iter()
            .find(|row| row["muscle"] == "chest")
            .expect("chest");
        let triceps = load
            .iter()
            .find(|row| row["muscle"] == "triceps")
            .expect("triceps");
        assert_eq!(chest["tonnageKg"], 500.0);
        assert_eq!(triceps["tonnageKg"], 250.0);
    }

    #[test]
    fn toggl_mapping_preserves_running_entry() {
        let entry = toggl_time_entry_object(&json!({
            "id": 42,
            "description": "Проект",
            "start": "2026-07-17T10:00:00Z",
            "stop": null,
            "duration": -1,
            "at": "2026-07-17T10:00:00Z",
            "workspace_id": 7,
            "tags": ["deep-work"]
        }))
        .expect("map time entry");
        assert_eq!(entry["id"], "toggl-time-entry:42");
        assert_eq!(entry["propsJson"]["endedAt"], Value::Null);
        assert_eq!(entry["propsJson"]["source"], "imported");
    }

    #[test]
    fn leetcode_mapping_keeps_attempt_metadata_without_code() {
        let submission = leetcode_submission_object(&json!({
            "id": "1972542025",
            "title": "Contains Duplicate",
            "titleSlug": "contains-duplicate",
            "statusDisplay": "Wrong Answer",
            "lang": "javascript",
            "timestamp": "1775606400",
            "url": "/submissions/detail/1972542025/",
            "runtime": "N/A",
            "memory": "N/A"
        }))
        .expect("map submission");
        assert_eq!(submission["id"], "leetcode-submission:1972542025");
        assert_eq!(submission["propsJson"]["status"], "Wrong Answer");
        assert_eq!(submission["propsJson"]["accepted"], false);
        assert!(submission["propsJson"].get("code").is_none());
    }

    #[test]
    fn leetcode_incremental_page_stops_at_overlap_cutoff() {
        // Regression: 2026-07-17. Every sync used to walk the complete submission history.
        let cutoff = DateTime::<Utc>::from_timestamp(150, 0).expect("cutoff");
        let recent = json!({ "timestamp": "200" });
        let old = json!({ "timestamp": "100" });
        let page = vec![recent.clone(), old.clone()];

        assert!(leetcode_page_reached_cutoff(&page, Some(cutoff)));
        assert!(leetcode_submission_is_new_enough(&recent, Some(cutoff)));
        assert!(!leetcode_submission_is_new_enough(&old, Some(cutoff)));
        assert!(!leetcode_page_reached_cutoff(&page, None));
    }

    #[test]
    fn duplicate_provider_sync_does_not_wait_for_the_first_one() {
        // Regression: 2026-07-17. Manual sync queued behind startup sync and then repeated it.
        let lock = sync_lock(Provider::Leetcode.sync_lock_index());
        let first = lock.try_lock().expect("first sync owns lock");
        assert!(lock.try_lock().is_err());
        drop(first);
        assert!(lock.try_lock().is_ok());
    }

    #[test]
    fn provider_requests_follow_official_auth_contracts() {
        let client = http_client().expect("client");
        let hevy = authenticated_get(
            &client,
            Provider::Hevy,
            "https://api.hevyapp.com/v1/workouts".to_string(),
            "test-hevy-key",
        )
        .build()
        .expect("hevy request");
        assert_eq!(hevy.url().path(), "/v1/workouts");
        assert_eq!(hevy.headers()["api-key"], "test-hevy-key");
        assert!(hevy.headers().get("authorization").is_none());

        let toggl = authenticated_get(
            &client,
            Provider::Toggl,
            "https://api.track.toggl.com/api/v9/me/time_entries".to_string(),
            "test-token",
        )
        .build()
        .expect("toggl request");
        assert_eq!(toggl.url().path(), "/api/v9/me/time_entries");
        assert_eq!(
            toggl.headers()["authorization"],
            "Basic dGVzdC10b2tlbjphcGlfdG9rZW4="
        );
    }

    #[test]
    fn scheduler_interval_is_bounded() {
        assert_eq!(validate_interval(60), Ok(60));
        assert!(validate_interval(1).is_err());
    }

    #[test]
    fn toggl_pagination_moves_before_the_oldest_entry() {
        let cursor = toggl_before_cursor(&[
            json!({ "start": "2026-07-17T10:00:00Z" }),
            json!({ "start": "2020-01-01T00:00:00Z" }),
        ])
        .expect("cursor");
        assert_eq!(cursor, "2019-12-31T23:59:59.999+00:00");
    }
}
