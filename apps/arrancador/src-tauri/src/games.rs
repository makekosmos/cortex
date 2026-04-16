use crate::db::GlobalDb;
pub use crate::domain::games::{Game, NewGame, UpdateGame};
use crate::services::fs::StdFileSystem;
use crate::services::games as games_service;

#[tauri::command]
pub fn get_game(id: String) -> Result<Option<Game>, String> {
    games_service::get_game(&GlobalDb, id)
}

#[tauri::command]
pub fn add_game(game: NewGame) -> Result<Game, String> {
    games_service::add_game(&GlobalDb, game)
}

#[tauri::command]
pub fn add_games_batch(games: Vec<NewGame>) -> Result<Vec<Game>, String> {
    games_service::add_games_batch(&GlobalDb, games)
}

#[tauri::command]
pub fn get_all_games() -> Result<Vec<Game>, String> {
    games_service::get_all_games(&GlobalDb)
}

#[tauri::command]
pub fn get_favorites() -> Result<Vec<Game>, String> {
    games_service::get_favorites(&GlobalDb)
}

#[tauri::command]
pub fn update_game(update: UpdateGame) -> Result<Game, String> {
    games_service::update_game(&GlobalDb, update)
}

#[tauri::command]
pub fn toggle_favorite(id: String) -> Result<Game, String> {
    games_service::toggle_favorite(&GlobalDb, id)
}

#[tauri::command]
pub fn delete_game(id: String) -> Result<(), String> {
    games_service::delete_game(&GlobalDb, id)
}

#[tauri::command]
pub fn record_game_launch(id: String) -> Result<Game, String> {
    games_service::record_game_launch(&GlobalDb, id)
}

#[tauri::command]
pub fn search_games(query: String) -> Result<Vec<Game>, String> {
    games_service::search_games(&GlobalDb, query)
}

#[tauri::command]
pub fn game_exists_by_path(exe_path: String) -> Result<bool, String> {
    games_service::game_exists_by_path(&GlobalDb, exe_path)
}

#[tauri::command]
pub fn resolve_shortcut_target(path: String) -> Result<String, String> {
    games_service::resolve_shortcut_target(path)
}

#[tauri::command]
pub fn is_game_installed(id: String) -> Result<bool, String> {
    games_service::is_game_installed(&GlobalDb, &StdFileSystem, id)
}

#[tauri::command]
pub fn get_running_instances(id: String) -> Result<u32, String> {
    games_service::get_running_instances(&GlobalDb, id)
}

#[tauri::command]
pub fn kill_game_processes(id: String) -> Result<u32, String> {
    games_service::kill_game_processes(&GlobalDb, id)
}

#[tauri::command]
pub async fn launch_game(id: String) -> Result<(), String> {
    games_service::launch_game(&GlobalDb, id).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::{set_test_db, TestDbGuard, TEST_DB_MUTEX};
    use rusqlite::{params, Connection};

    fn setup_db() -> TestDbGuard {
        let conn = Connection::open_in_memory().expect("open db");
        conn.execute(
            "CREATE TABLE games (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                exe_path TEXT NOT NULL UNIQUE,
                exe_name TEXT NOT NULL,
                rawg_id INTEGER,
                description TEXT,
                released TEXT,
                background_image TEXT,
                metacritic INTEGER,
                rating REAL,
                genres TEXT,
                platforms TEXT,
                developers TEXT,
                publishers TEXT,
                cover_image TEXT,
                icon_image TEXT,
                is_favorite INTEGER DEFAULT 0,
                play_count INTEGER DEFAULT 0,
                total_playtime INTEGER DEFAULT 0,
                last_played TEXT,
                date_added TEXT NOT NULL,
                backup_enabled INTEGER DEFAULT 0,
                last_backup TEXT,
                backup_count INTEGER DEFAULT 0,
                save_path TEXT,
                save_path_checked INTEGER DEFAULT 0,
                user_rating INTEGER,
                user_note TEXT,
                play_status TEXT NOT NULL DEFAULT 'not_started'
            )",
            [],
        )
        .expect("create games table");
        conn.execute(
            "CREATE TABLE settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )",
            [],
        )
        .expect("create settings table");
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('backup_directory', '')",
            [],
        )
        .expect("insert settings");

        set_test_db(conn)
    }

    #[test]
    fn add_game_and_fetch() {
        let _lock = TEST_DB_MUTEX.lock().unwrap();
        let _db_guard = setup_db();

        let added = add_game(NewGame {
            name: "Test Game".to_string(),
            exe_path: "C:\\Games\\test.exe".to_string(),
            exe_name: "test.exe".to_string(),
        })
        .expect("add game");

        let fetched = get_game(added.id.clone())
            .expect("get game")
            .expect("game exists");

        assert_eq!(fetched.name, "Test Game");
        assert_eq!(fetched.exe_path, "C:\\Games\\test.exe");
        assert!(!fetched.is_favorite);
        assert_eq!(fetched.play_count, 0);
    }

    #[test]
    fn update_game_updates_fields_and_save_path_checked() {
        let _lock = TEST_DB_MUTEX.lock().unwrap();
        let _db_guard = setup_db();

        let added = add_game(NewGame {
            name: "Original".to_string(),
            exe_path: "C:\\Games\\original.exe".to_string(),
            exe_name: "original.exe".to_string(),
        })
        .expect("add game");

        let updated = update_game(UpdateGame {
            id: added.id.clone(),
            name: Some("Updated".to_string()),
            exe_path: None,
            description: None,
            cover_image: None,
            icon_image: None,
            is_favorite: Some(true),
            backup_enabled: None,
            save_path: Some("C:\\Saves\\updated".to_string()),
            rawg_id: None,
            released: None,
            background_image: None,
            metacritic: None,
            rating: None,
            genres: None,
            platforms: None,
            developers: None,
            publishers: None,
            play_status: None,
            user_rating: Some(5),
            user_note: Some("solid".to_string()),
        })
        .expect("update game");

        assert_eq!(updated.name, "Updated");
        assert!(updated.is_favorite);
        assert_eq!(updated.save_path.as_deref(), Some("C:\\Saves\\updated"));
        assert_eq!(updated.user_rating, Some(5));
        assert_eq!(updated.user_note.as_deref(), Some("solid"));

        let checked: i32 = crate::database::with_db(|conn| {
            conn.query_row(
                "SELECT save_path_checked FROM games WHERE id = ?1",
                params![added.id],
                |row| row.get(0),
            )
        })
        .expect("query save_path_checked");
        assert_eq!(checked, 1);
    }

    #[test]
    fn get_all_games_sorted_by_name() {
        let _lock = TEST_DB_MUTEX.lock().unwrap();
        let _db_guard = setup_db();

        add_game(NewGame {
            name: "Zeta".to_string(),
            exe_path: "C:\\Games\\zeta.exe".to_string(),
            exe_name: "zeta.exe".to_string(),
        })
        .expect("add zeta");
        add_game(NewGame {
            name: "Alpha".to_string(),
            exe_path: "C:\\Games\\alpha.exe".to_string(),
            exe_name: "alpha.exe".to_string(),
        })
        .expect("add alpha");

        let games = get_all_games().expect("get all games");
        let names: Vec<String> = games.into_iter().map(|g| g.name).collect();
        assert_eq!(names, vec!["Alpha", "Zeta"]);
    }

    #[test]
    fn delete_game_removes_row() {
        let _lock = TEST_DB_MUTEX.lock().unwrap();
        let _db_guard = setup_db();

        let added = add_game(NewGame {
            name: "To Remove".to_string(),
            exe_path: "C:\\Games\\remove.exe".to_string(),
            exe_name: "remove.exe".to_string(),
        })
        .expect("add game");

        delete_game(added.id.clone()).expect("delete game");

        let fetched = get_game(added.id).expect("get game");
        assert!(fetched.is_none());
    }
}
