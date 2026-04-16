use crate::database::with_db;
use crate::games::Game;
use reqwest::Client;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::time::Duration;

const RAWG_API_BASE: &str = "https://api.rawg.io/api";
const RAWG_TIMEOUT_SECS: u64 = 15;

#[derive(Debug, Serialize, Deserialize)]
pub struct RawgSearchResult {
    pub count: i32,
    pub results: Vec<RawgGame>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawgGame {
    pub id: i64,
    pub name: String,
    pub slug: String,
    pub released: Option<String>,
    pub background_image: Option<String>,
    pub metacritic: Option<i32>,
    pub rating: Option<f64>,
    pub ratings_count: Option<i32>,
    pub genres: Option<Vec<RawgGenre>>,
    pub platforms: Option<Vec<RawgPlatformWrapper>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawgGenre {
    pub id: i64,
    pub name: String,
    pub slug: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawgPlatformWrapper {
    pub platform: RawgPlatform,
    pub requirements_en: Option<String>,
    pub requirements_ru: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawgPlatform {
    pub id: i64,
    pub name: String,
    pub slug: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawgGameDetails {
    pub id: i64,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub description_raw: Option<String>,
    pub released: Option<String>,
    pub background_image: Option<String>,
    pub background_image_additional: Option<String>,
    pub metacritic: Option<i32>,
    pub rating: Option<f64>,
    pub genres: Option<Vec<RawgGenre>>,
    pub platforms: Option<Vec<RawgPlatformWrapper>>,
    pub developers: Option<Vec<RawgDeveloper>>,
    pub publishers: Option<Vec<RawgPublisher>>,
    pub stores: Option<Vec<RawgStoreWrapper>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawgDeveloper {
    pub id: i64,
    pub name: String,
    pub slug: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawgPublisher {
    pub id: i64,
    pub name: String,
    pub slug: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawgStoreWrapper {
    pub id: i64,
    pub url: Option<String>,
    pub store: Option<RawgStore>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawgStore {
    pub id: i64,
    pub name: String,
    pub slug: String,
    pub domain: Option<String>,
}

fn get_api_key() -> String {
    with_db(|conn| {
        let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = 'rawg_api_key'")?;
        let key: String = stmt.query_row([], |row| row.get(0)).unwrap_or_default();
        Ok(key)
    })
    .unwrap_or_default()
}

fn rawg_client() -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(RAWG_TIMEOUT_SECS))
        .build()
        .map_err(|e| format!("Failed to build RAWG client: {e}"))
}

#[tauri::command]
pub async fn search_rawg(query: String) -> Result<Vec<RawgGame>, String> {
    let api_key = get_api_key();

    let url = if api_key.is_empty() {
        format!(
            "{}/games?search={}&page_size=10",
            RAWG_API_BASE,
            urlencoding::encode(&query)
        )
    } else {
        format!(
            "{}/games?key={}&search={}&page_size=10",
            RAWG_API_BASE,
            api_key,
            urlencoding::encode(&query)
        )
    };

    let client = rawg_client()?;
    let response = client
        .get(&url)
        .header("User-Agent", "Arrancador/0.1.0")
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                "RAWG request timed out".to_string()
            } else {
                format!("Network error: {}", e)
            }
        })?;

    if !response.status().is_success() {
        return Err(format!("API error: {}", response.status()));
    }

    let result: RawgSearchResult = response
        .json()
        .await
        .map_err(|e| format!("Parse error: {}", e))?;

    Ok(result.results)
}

#[tauri::command]
pub async fn get_rawg_game_details(rawg_id: i64) -> Result<RawgGameDetails, String> {
    let api_key = get_api_key();

    let url = if api_key.is_empty() {
        format!("{}/games/{}", RAWG_API_BASE, rawg_id)
    } else {
        format!("{}/games/{}?key={}", RAWG_API_BASE, rawg_id, api_key)
    };

    let client = rawg_client()?;
    let response = client
        .get(&url)
        .header("User-Agent", "Arrancador/0.1.0")
        .send()
        .await
        .map_err(|e| {
            if e.is_timeout() {
                "RAWG request timed out".to_string()
            } else {
                format!("Network error: {}", e)
            }
        })?;

    if !response.status().is_success() {
        return Err(format!("API error: {}", response.status()));
    }

    let details: RawgGameDetails = response
        .json()
        .await
        .map_err(|e| format!("Parse error: {}", e))?;

    Ok(details)
}

#[tauri::command]
pub async fn apply_rawg_metadata(
    game_id: String,
    rawg_id: i64,
    rename: bool,
) -> Result<Game, String> {
    let details = get_rawg_game_details(rawg_id).await?;

    let genres = details.genres.as_ref().map(|g| {
        g.iter()
            .map(|genre| genre.name.clone())
            .collect::<Vec<_>>()
            .join(", ")
    });

    let platforms = details.platforms.as_ref().map(|p| {
        p.iter()
            .map(|pw| pw.platform.name.clone())
            .collect::<Vec<_>>()
            .join(", ")
    });

    let developers = details.developers.as_ref().map(|d| {
        d.iter()
            .map(|dev| dev.name.clone())
            .collect::<Vec<_>>()
            .join(", ")
    });

    let publishers = details.publishers.as_ref().map(|p| {
        p.iter()
            .map(|pub_| pub_.name.clone())
            .collect::<Vec<_>>()
            .join(", ")
    });

    let description = details.description_raw.or(details.description);
    let new_name = if rename {
        Some(details.name.clone())
    } else {
        None
    };

    // Update game in database
    with_db(|conn| {
        conn.execute(
            "UPDATE games SET
                name = COALESCE(?1, name),
                rawg_id = ?2,
                description = ?3,
                released = ?4,
                background_image = ?5,
                metacritic = ?6,
                rating = ?7,
                genres = ?8,
                platforms = ?9,
                developers = ?10,
                publishers = ?11
            WHERE id = ?12",
            params![
                new_name,
                rawg_id,
                description,
                details.released,
                details.background_image,
                details.metacritic,
                details.rating,
                genres,
                platforms,
                developers,
                publishers,
                game_id
            ],
        )?;

        let mut stmt = conn.prepare(
            "SELECT id, name, exe_path, exe_name, rawg_id, description, released,
             background_image, metacritic, rating, genres, platforms, developers, publishers,
             cover_image, icon_image, is_favorite, play_count, total_playtime, last_played, date_added,
             backup_enabled, last_backup, backup_count, save_path, user_rating, user_note, play_status
              FROM games WHERE id = ?1",
        )?;

        stmt.query_row(params![game_id], |row| {
            let play_status: String = row
                .get::<_, Option<String>>(27)?
                .unwrap_or_else(|| "not_started".to_string());
            Ok(Game {
                id: row.get(0)?,
                name: row.get(1)?,
                exe_path: row.get(2)?,
                exe_name: row.get(3)?,
                rawg_id: row.get(4)?,
                description: row.get(5)?,
                released: row.get(6)?,
                background_image: row.get(7)?,
                metacritic: row.get(8)?,
                rating: row.get(9)?,
                genres: row.get(10)?,
                platforms: row.get(11)?,
                developers: row.get(12)?,
                publishers: row.get(13)?,
                cover_image: row.get(14)?,
                icon_image: row.get(15)?,
                is_favorite: row.get::<_, i32>(16)? == 1,
                play_count: row.get(17)?,
                total_playtime: row.get(18)?,
                last_played: row.get(19)?,
                date_added: row.get(20)?,
                backup_enabled: row.get::<_, i32>(21)? == 1,
                last_backup: row.get(22)?,
                backup_count: row.get(23)?,
                save_path: row.get(24)?,
                user_rating: row.get(25)?,
                user_note: row.get(26)?,
                play_status,
            })
        })
    })
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_rawg_api_key(key: String) -> Result<(), String> {
    with_db(|conn| {
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES ('rawg_api_key', ?1)",
            params![key],
        )?;
        Ok(())
    })
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_rawg_api_key() -> Result<String, String> {
    Ok(get_api_key())
}
