use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game {
    pub id: String,
    pub name: String,
    pub exe_path: String,
    pub exe_name: String,
    pub play_status: String,

    // RAWG metadata
    pub rawg_id: Option<i64>,
    pub description: Option<String>,
    pub released: Option<String>,
    pub background_image: Option<String>,
    pub metacritic: Option<i32>,
    pub rating: Option<f64>,
    pub genres: Option<String>,
    pub platforms: Option<String>,
    pub developers: Option<String>,
    pub publishers: Option<String>,

    // Local metadata
    pub cover_image: Option<String>,
    pub icon_image: Option<String>,
    pub is_favorite: bool,
    pub play_count: i32,
    pub total_playtime: i64,
    pub last_played: Option<String>,
    pub date_added: String,

    // Backup
    pub backup_enabled: bool,
    pub last_backup: Option<String>,
    pub backup_count: i32,
    pub save_path: Option<String>,

    pub user_rating: Option<i32>,
    pub user_note: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NewGame {
    pub name: String,
    pub exe_path: String,
    pub exe_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateGame {
    pub id: String,
    pub name: Option<String>,
    pub exe_path: Option<String>,
    pub description: Option<String>,
    pub cover_image: Option<String>,
    pub icon_image: Option<String>,
    pub is_favorite: Option<bool>,
    pub backup_enabled: Option<bool>,
    pub save_path: Option<String>,
    pub rawg_id: Option<i64>,
    pub released: Option<String>,
    pub background_image: Option<String>,
    pub metacritic: Option<i32>,
    pub rating: Option<f64>,
    pub genres: Option<String>,
    pub platforms: Option<String>,
    pub developers: Option<String>,
    pub publishers: Option<String>,
    pub play_status: Option<String>,
    pub user_rating: Option<i32>,
    pub user_note: Option<String>,
}
