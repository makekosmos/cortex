// RAWG provider client and typed Game mutation boundary.
use super::game_facade::GameFacade;
use ark_core::canonical_types::game::{GameProviderRef, GameRecord, GameUpsertCommand};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const RAWG_BASE_URL: &str = "https://api.rawg.io/api";
const USER_AGENT: &str = "Kosmos/0.1 Kepler";
const PAGE_SIZE: u32 = 20;

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
    #[error("ark host request failed: {0}")]
    ArkHost(String),
}

fn build_client() -> Result<reqwest::Client, RawgError> {
    Ok(reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(std::time::Duration::from_secs(30))
        .build()?)
}
pub async fn search(query: &str, api_key: &str) -> Result<Vec<RawgGame>, RawgError> {
    search_with_base(query, api_key, RAWG_BASE_URL).await
}
async fn search_with_base(
    query: &str,
    api_key: &str,
    base: &str,
) -> Result<Vec<RawgGame>, RawgError> {
    if api_key.is_empty() {
        return Err(RawgError::NoApiKey);
    }
    let url = format!(
        "{base}/games?search={}&key={}&page_size={PAGE_SIZE}",
        urlencoding::encode(query),
        urlencoding::encode(api_key)
    );
    let response = build_client()?.get(url).send().await?;
    let status = response.status();
    let body = response.text().await?;
    if !status.is_success() {
        return Err(RawgError::Status(status.as_u16(), body));
    }
    Ok(serde_json::from_str::<RawgSearchResponse>(&body)?.results)
}
pub async fn get_details(rawg_id: u32, api_key: &str) -> Result<RawgGameDetail, RawgError> {
    get_details_with_base(rawg_id, api_key, RAWG_BASE_URL).await
}
async fn get_details_with_base(
    rawg_id: u32,
    api_key: &str,
    base: &str,
) -> Result<RawgGameDetail, RawgError> {
    if api_key.is_empty() {
        return Err(RawgError::NoApiKey);
    }
    let response = build_client()?
        .get(format!(
            "{base}/games/{rawg_id}?key={}",
            urlencoding::encode(api_key)
        ))
        .send()
        .await?;
    let status = response.status();
    let body = response.text().await?;
    if !status.is_success() {
        return Err(RawgError::Status(status.as_u16(), body));
    }
    Ok(serde_json::from_str(&body)?)
}

#[async_trait::async_trait]
pub trait GameMutationFacade: Send + Sync {
    async fn mutation_record(&self, game_id: &str) -> Result<GameRecord, String>;
    async fn upsert(&self, command: GameUpsertCommand) -> Result<GameRecord, String>;
}

pub async fn apply_to_game(
    facade: &GameFacade<'_>,
    game_id: &str,
    rawg_id: u32,
    api_key: &str,
) -> Result<(), RawgError> {
    apply_to_game_with_base(facade, game_id, rawg_id, api_key, RAWG_BASE_URL).await
}
async fn apply_to_game_with_base<F: GameMutationFacade>(
    facade: &F,
    game_id: &str,
    rawg_id: u32,
    api_key: &str,
    base: &str,
) -> Result<(), RawgError> {
    let details = get_details_with_base(rawg_id, api_key, base).await?;
    let existing = facade
        .mutation_record(game_id)
        .await
        .map_err(RawgError::ArkHost)?;
    let command = GameUpsertCommand {
        id: existing.id.clone(),
        title: existing.title.clone(),
        play_status: existing.play_status.clone(),
        user_rating: existing.user_rating,
        genres: details.genres.iter().map(|g| g.name.clone()).collect(),
        platforms: details
            .platforms
            .iter()
            .map(|p| p.platform.name.clone())
            .collect(),
        released: details.released.clone().or(existing.released.clone()),
        description: details
            .description_raw
            .clone()
            .or(details.description.clone())
            .or(existing.description.clone()),
        extensions: existing.extensions.clone(),
        created_at: existing.created_at.clone(),
        updated_at: chrono::Utc::now().to_rfc3339(),
        deleted_at: existing.deleted_at.clone(),
        links: existing.links.clone(),
        local: existing.local.clone(),
        quarantine: ark_core::canonical_types::game::GameQuarantine {
            provider_refs: vec![GameProviderRef {
                provider: "rawg".into(),
                provider_id: rawg_id.to_string(),
            }],
            fields: details
                .background_image
                .into_iter()
                .map(|_| "background_image".into())
                .collect(),
        },
        device_id: None,
    };
    facade.upsert(command).await.map_err(RawgError::ArkHost)?;
    Ok(())
}

#[async_trait::async_trait]
impl GameMutationFacade for GameFacade<'_> {
    async fn mutation_record(&self, game_id: &str) -> Result<GameRecord, String> {
        self.mutation_record(game_id).await
    }
    async fn upsert(&self, command: GameUpsertCommand) -> Result<GameRecord, String> {
        self.upsert(command).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn provider_quarantine_is_typed() {
        let q = ark_core::canonical_types::game::GameQuarantine {
            provider_refs: vec![GameProviderRef {
                provider: "rawg".into(),
                provider_id: "1".into(),
            }],
            fields: vec!["background_image".into()],
        };
        assert_eq!(q.provider_refs[0].provider, "rawg");
    }
}
