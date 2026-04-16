use crate::backup::import_existing_backups_for_game;
use crate::db::Db;
use crate::domain::games::{Game, NewGame, UpdateGame};
use crate::services::fs::FileSystem;
use chrono::Utc;
use rusqlite::{params, Result};
#[cfg(target_os = "windows")]
use std::ffi::OsStr;
use std::fs;
#[cfg(target_os = "windows")]
use std::os::windows::ffi::OsStrExt;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use sysinfo::{ProcessesToUpdate, System};
use uuid::Uuid;
#[cfg(target_os = "windows")]
use windows::core::{Interface, PCWSTR};
#[cfg(target_os = "windows")]
use windows::Win32::Storage::FileSystem::WIN32_FIND_DATAW;
#[cfg(target_os = "windows")]
use windows::Win32::System::Com::IPersistFile;
#[cfg(target_os = "windows")]
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
    COINIT_APARTMENTTHREADED, STGM_READ,
};
#[cfg(target_os = "windows")]
use windows::Win32::System::Threading::{
    CREATE_BREAKAWAY_FROM_JOB, CREATE_NEW_PROCESS_GROUP, DETACHED_PROCESS,
};
#[cfg(target_os = "windows")]
use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};

const GAME_PATH_TOKEN: &str = "{PATHTOGAME}";
const GAME_SELECT: &str = "SELECT id, name, exe_path, exe_name, rawg_id, description, released,
             background_image, metacritic, rating, genres, platforms, developers, publishers,
             cover_image, icon_image, is_favorite, play_count, total_playtime, last_played, date_added,
             backup_enabled, last_backup, backup_count, save_path, user_rating, user_note, play_status
             FROM games";

fn tokenise_save_path_if_possible(
    conn: &rusqlite::Connection,
    game_id: &str,
    save_path: &str,
) -> String {
    if save_path.contains(GAME_PATH_TOKEN) {
        return save_path.to_string();
    }

    let save_path_pb = PathBuf::from(save_path);
    if !save_path_pb.is_absolute() || !save_path_pb.exists() {
        return save_path.to_string();
    }

    // If the chosen save path sits inside the game's directory, store it as a template so it
    // survives moving the game folder (as long as exe_path is updated).
    let exe_path: Option<String> = conn
        .prepare("SELECT exe_path FROM games WHERE id = ?1")
        .and_then(|mut stmt| stmt.query_row(params![game_id], |row| row.get(0)))
        .ok();
    let Some(exe_path) = exe_path else {
        return save_path.to_string();
    };
    let Some(game_dir) = Path::new(&exe_path).parent() else {
        return save_path.to_string();
    };

    let Ok(game_dir) = fs::canonicalize(game_dir) else {
        return save_path.to_string();
    };
    let Ok(save_path_pb) = fs::canonicalize(save_path_pb) else {
        return save_path.to_string();
    };

    let Ok(relative) = save_path_pb.strip_prefix(&game_dir) else {
        return save_path.to_string();
    };

    if relative.as_os_str().is_empty() {
        return GAME_PATH_TOKEN.to_string();
    }

    let mut out = String::from(GAME_PATH_TOKEN);
    out.push(std::path::MAIN_SEPARATOR);
    out.push_str(&relative.to_string_lossy());
    out
}

fn map_game_row(row: &rusqlite::Row) -> Result<Game> {
    let play_status: String = row.get::<_, Option<String>>(27)?.unwrap_or_else(|| {
        "not_started".to_string()
    });

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
}

fn fetch_game_by_id(conn: &rusqlite::Connection, id: &str) -> Result<Game> {
    let mut stmt = conn.prepare(&format!("{GAME_SELECT} WHERE id = ?1"))?;
    stmt.query_row(params![id], map_game_row)
}

fn fetch_exe_path<D: Db>(db: &D, id: &str) -> Result<String, String> {
    db.with_conn(|conn| {
        let mut stmt = conn.prepare("SELECT exe_path FROM games WHERE id = ?1")?;
        let path: String = stmt.query_row(params![id], |row| row.get(0))?;
        Ok(path)
    })
    .map_err(|e| e.to_string())
}

pub fn get_game<D: Db>(db: &D, id: String) -> Result<Option<Game>, String> {
    db.with_conn(|conn| {
        let mut stmt = conn.prepare(&format!("{GAME_SELECT} WHERE id = ?1"))?;
        let game = stmt.query_row(params![id], map_game_row).ok();
        Ok(game)
    })
    .map_err(|e| e.to_string())
}

pub fn add_game<D: Db>(db: &D, game: NewGame) -> Result<Game, String> {
    let id = Uuid::new_v4().to_string();
    let date_added = Utc::now().to_rfc3339();
    let game_name = game.name.clone();

    db.with_conn(|conn| {
        conn.execute(
            "INSERT INTO games (id, name, exe_path, exe_name, date_added) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, game.name, game.exe_path, game.exe_name, date_added],
        )?;
        Ok(())
    })
    .map_err(|e| e.to_string())?;

    if let Err(e) = import_existing_backups_for_game(&id, &game_name) {
        eprintln!("Failed to import backups for {}: {}", id, e);
    }

    db.with_conn(|conn| fetch_game_by_id(conn, &id))
        .map_err(|e| e.to_string())
}

pub fn add_games_batch<D: Db>(db: &D, games: Vec<NewGame>) -> Result<Vec<Game>, String> {
    if games.is_empty() {
        return Ok(Vec::new());
    }

    let inserted = match db.with_conn(|conn| {
        conn.execute_batch("BEGIN IMMEDIATE")?;
        let mut inserted = Vec::new();
        {
            let mut stmt = conn.prepare(
                "INSERT INTO games (id, name, exe_path, exe_name, date_added) VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;
            for game in games {
                let id = Uuid::new_v4().to_string();
                let date_added = Utc::now().to_rfc3339();
                let game_name = game.name.clone();
                match stmt.execute(params![id, game.name, game.exe_path, game.exe_name, date_added]) {
                    Ok(_) => inserted.push((id, game_name)),
                    Err(e) => {
                        if !e.to_string().contains("UNIQUE constraint failed") {
                            eprintln!("Error adding game: {}", e);
                        }
                    }
                }
            }
        }
        conn.execute_batch("COMMIT")?;
        Ok(inserted)
    }) {
        Ok(inserted) => inserted,
        Err(e) => {
            eprintln!("Error adding game batch: {}", e);
            return Ok(Vec::new());
        }
    };

    let mut added_games = Vec::new();
    for (id, game_name) in inserted {
        if let Err(e) = import_existing_backups_for_game(&id, &game_name) {
            eprintln!("Failed to import backups for {}: {}", id, e);
        }

        match db.with_conn(|conn| fetch_game_by_id(conn, &id)) {
            Ok(game) => added_games.push(game),
            Err(e) => eprintln!("Error fetching new game {}: {}", id, e),
        }
    }

    Ok(added_games)
}

pub fn get_all_games<D: Db>(db: &D) -> Result<Vec<Game>, String> {
    db.with_conn(|conn| {
        let mut stmt = conn.prepare(&format!("{GAME_SELECT} ORDER BY name ASC"))?;

        let games = stmt
            .query_map([], map_game_row)?
            .filter_map(|r| r.ok())
            .collect();

        Ok(games)
    })
    .map_err(|e| e.to_string())
}

pub fn get_favorites<D: Db>(db: &D) -> Result<Vec<Game>, String> {
    db.with_conn(|conn| {
        let mut stmt = conn.prepare(&format!(
            "{GAME_SELECT} WHERE is_favorite = 1 ORDER BY name ASC"
        ))?;

        let games = stmt
            .query_map([], map_game_row)?
            .filter_map(|r| r.ok())
            .collect();

        Ok(games)
    })
    .map_err(|e| e.to_string())
}

pub fn update_game<D: Db>(db: &D, update: UpdateGame) -> Result<Game, String> {
    db.with_conn(|conn| {
        let mut updates = Vec::new();
        let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(ref name) = update.name {
            updates.push("name = ?");
            params_vec.push(Box::new(name.clone()));
        }
        if let Some(ref exe_path) = update.exe_path {
            let normalized = exe_path.trim();
            if normalized.is_empty() {
                return Err(rusqlite::Error::InvalidParameterName(
                    "exe_path cannot be empty".to_string(),
                ));
            }
            let exe_name = Path::new(normalized)
                .file_name()
                .and_then(|value| value.to_str())
                .ok_or_else(|| rusqlite::Error::InvalidColumnName("Invalid exe path".to_string()))?
                .to_string();

            updates.push("exe_path = ?");
            params_vec.push(Box::new(normalized.to_string()));
            updates.push("exe_name = ?");
            params_vec.push(Box::new(exe_name));
        }
        if let Some(ref desc) = update.description {
            updates.push("description = ?");
            params_vec.push(Box::new(desc.clone()));
        }
        if let Some(ref cover) = update.cover_image {
            updates.push("cover_image = ?");
            params_vec.push(Box::new(cover.clone()));
        }
        if let Some(ref icon) = update.icon_image {
            updates.push("icon_image = ?");
            params_vec.push(Box::new(icon.clone()));
        }
        if let Some(fav) = update.is_favorite {
            updates.push("is_favorite = ?");
            params_vec.push(Box::new(if fav { 1 } else { 0 }));
        }
        if let Some(backup) = update.backup_enabled {
            updates.push("backup_enabled = ?");
            params_vec.push(Box::new(if backup { 1 } else { 0 }));
        }
        if let Some(ref save_path) = update.save_path {
            updates.push("save_path = ?");
            let normalized = if save_path.trim().is_empty() {
                None
            } else {
                Some(tokenise_save_path_if_possible(conn, &update.id, save_path))
            };
            let checked = normalized.is_some();
            params_vec.push(Box::new(normalized));
            updates.push("save_path_checked = ?");
            params_vec.push(Box::new(if checked { 1 } else { 0 }));
        }
        if let Some(rawg_id) = update.rawg_id {
            updates.push("rawg_id = ?");
            params_vec.push(Box::new(rawg_id));
        }
        if let Some(ref released) = update.released {
            updates.push("released = ?");
            params_vec.push(Box::new(released.clone()));
        }
        if let Some(ref bg) = update.background_image {
            updates.push("background_image = ?");
            params_vec.push(Box::new(bg.clone()));
        }
        if let Some(mc) = update.metacritic {
            updates.push("metacritic = ?");
            params_vec.push(Box::new(mc));
        }
        if let Some(rating) = update.rating {
            updates.push("rating = ?");
            params_vec.push(Box::new(rating));
        }
        if let Some(ref genres) = update.genres {
            updates.push("genres = ?");
            params_vec.push(Box::new(genres.clone()));
        }
        if let Some(ref platforms) = update.platforms {
            updates.push("platforms = ?");
            params_vec.push(Box::new(platforms.clone()));
        }
        if let Some(ref devs) = update.developers {
            updates.push("developers = ?");
            params_vec.push(Box::new(devs.clone()));
        }
        if let Some(ref pubs) = update.publishers {
            updates.push("publishers = ?");
            params_vec.push(Box::new(pubs.clone()));
        }
        if let Some(user_rating) = update.user_rating {
            updates.push("user_rating = ?");
            params_vec.push(Box::new(user_rating));
        }
        if let Some(ref user_note) = update.user_note {
            updates.push("user_note = ?");
            params_vec.push(Box::new(user_note.clone()));
        }
        if let Some(ref play_status) = update.play_status {
            updates.push("play_status = ?");
            params_vec.push(Box::new(play_status.clone()));
        }

        if updates.is_empty() {
            return fetch_game_by_id(conn, &update.id);
        }

        params_vec.push(Box::new(update.id.clone()));

        let sql = format!("UPDATE games SET {} WHERE id = ?", updates.join(", "));

        let params_refs: Vec<&dyn rusqlite::ToSql> =
            params_vec.iter().map(|p| p.as_ref()).collect();
        conn.execute(&sql, params_refs.as_slice())?;

        fetch_game_by_id(conn, &update.id)
    })
    .map_err(|e| e.to_string())
}

pub fn toggle_favorite<D: Db>(db: &D, id: String) -> Result<Game, String> {
    db.with_conn(|conn| {
        conn.execute(
            "UPDATE games SET is_favorite = CASE WHEN is_favorite = 1 THEN 0 ELSE 1 END WHERE id = ?1",
            params![id],
        )?;

        fetch_game_by_id(conn, &id)
    })
    .map_err(|e| e.to_string())
}

pub fn delete_game<D: Db>(db: &D, id: String) -> Result<(), String> {
    db.with_conn(|conn| {
        conn.execute("DELETE FROM games WHERE id = ?1", params![id])?;
        Ok(())
    })
    .map_err(|e| e.to_string())
}

pub fn record_game_launch<D: Db>(db: &D, id: String) -> Result<Game, String> {
    let now = Utc::now().to_rfc3339();

    db.with_conn(|conn| {
        conn.execute(
            "UPDATE games SET play_count = play_count + 1, last_played = ?1 WHERE id = ?2",
            params![now, id],
        )?;

        fetch_game_by_id(conn, &id)
    })
    .map_err(|e| e.to_string())
}

pub fn search_games<D: Db>(db: &D, query: String) -> Result<Vec<Game>, String> {
    db.with_conn(|conn| {
        let pattern = format!("%{}%", query);
        let mut stmt = conn.prepare(&format!(
            "{GAME_SELECT} WHERE name LIKE ?1 OR exe_name LIKE ?1 ORDER BY name ASC"
        ))?;

        let games = stmt
            .query_map(params![pattern], map_game_row)?
            .filter_map(|r| r.ok())
            .collect();

        Ok(games)
    })
    .map_err(|e| e.to_string())
}

pub fn game_exists_by_path<D: Db>(db: &D, exe_path: String) -> Result<bool, String> {
    db.with_conn(|conn| {
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM games WHERE exe_path = ?1")?;
        let count: i32 = stmt.query_row(params![exe_path], |row| row.get(0))?;
        Ok(count > 0)
    })
    .map_err(|e| e.to_string())
}

pub fn resolve_shortcut_target(path: String) -> Result<String, String> {
    let input = PathBuf::from(&path);
    let is_shortcut = input
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.eq_ignore_ascii_case("lnk"))
        .unwrap_or(false);
    if !is_shortcut {
        return Ok(path);
    }

    #[cfg(target_os = "windows")]
    {
        let resolved = resolve_shortcut_windows(&input)?;
        Ok(resolved.to_string_lossy().to_string())
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(path)
    }
}

pub fn is_game_installed<D: Db, F: FileSystem>(db: &D, fs: &F, id: String) -> Result<bool, String> {
    let exe_path = fetch_exe_path(db, &id)?;
    Ok(fs.exists(Path::new(&exe_path)))
}

pub fn get_running_instances<D: Db>(db: &D, id: String) -> Result<u32, String> {
    let exe_path = fetch_exe_path(db, &id)?;

    let mut sys = System::new_all();
    sys.refresh_processes(ProcessesToUpdate::All, true);

    let target = PathBuf::from(exe_path);
    let mut count = 0u32;
    for process in sys.processes().values() {
        if let Some(path) = process.exe() {
            if paths_match(path, &target) {
                count += 1;
            }
        }
    }

    Ok(count)
}

pub fn kill_game_processes<D: Db>(db: &D, id: String) -> Result<u32, String> {
    let exe_path = fetch_exe_path(db, &id)?;

    let mut sys = System::new_all();
    sys.refresh_processes(ProcessesToUpdate::All, true);

    let target = PathBuf::from(exe_path);
    let mut killed = 0u32;
    for process in sys.processes().values() {
        if let Some(path) = process.exe() {
            if paths_match(path, &target) && process.kill() {
                killed += 1;
            }
        }
    }

    Ok(killed)
}

pub async fn launch_game<D: Db + Sync>(db: &D, id: String) -> Result<(), String> {
    let exe_path = fetch_exe_path(db, &id)?;

    tauri::async_runtime::spawn_blocking(move || spawn_game_process(&exe_path))
        .await
        .map_err(|e| e.to_string())??;

    record_game_launch(db, id.clone())?;

    Ok(())
}

fn paths_match(p1: &Path, p2: &Path) -> bool {
    if cfg!(target_os = "windows") {
        p1.to_string_lossy().to_lowercase() == p2.to_string_lossy().to_lowercase()
    } else {
        p1 == p2
    }
}

#[cfg(target_os = "windows")]
struct ComGuard;

#[cfg(target_os = "windows")]
impl Drop for ComGuard {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}

#[cfg(target_os = "windows")]
fn resolve_shortcut_windows(path: &PathBuf) -> Result<PathBuf, String> {
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED)
            .ok()
            .map_err(|e| e.to_string())?;
    }
    let _guard = ComGuard;

    let link: IShellLinkW = unsafe {
        CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).map_err(|e| e.to_string())?
    };
    let persist: IPersistFile = link
        .cast::<IPersistFile>()
        .map_err(|e: windows::core::Error| e.to_string())?;

    let wide: Vec<u16> = OsStr::new(path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        persist
            .Load(PCWSTR(wide.as_ptr()), STGM_READ)
            .map_err(|e: windows::core::Error| e.to_string())?;
    }

    let mut buffer = [0u16; 260];
    let mut data = WIN32_FIND_DATAW::default();
    unsafe {
        link.GetPath(&mut buffer, &mut data, 0)
            .map_err(|e: windows::core::Error| e.to_string())?;
    }

    let len = buffer.iter().position(|c| *c == 0).unwrap_or(buffer.len());
    let target = String::from_utf16_lossy(&buffer[..len]);
    if target.trim().is_empty() {
        return Err("Shortcut target is empty".to_string());
    }
    Ok(PathBuf::from(target))
}

fn spawn_game_process(exe_path: &str) -> Result<(), String> {
    let path = Path::new(exe_path);
    let parent = path.parent().unwrap_or(path);

    #[cfg(target_os = "windows")]
    {
        let mut command = std::process::Command::new(path);
        command.current_dir(parent);
        let flags = CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS | CREATE_BREAKAWAY_FROM_JOB;
        match command.creation_flags(flags.0).spawn() {
            Ok(_) => Ok(()),
            Err(_) => {
                let mut fallback = std::process::Command::new(path);
                fallback.current_dir(parent);
                let fallback_flags = CREATE_NEW_PROCESS_GROUP | DETACHED_PROCESS;
                fallback
                    .creation_flags(fallback_flags.0)
                    .spawn()
                    .map(|_| ())
                    .map_err(|e| format!("Failed to launch game: {}", e))
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        std::process::Command::new(path)
            .current_dir(parent)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Failed to launch game: {}", e))
    }
}

#[cfg(test)]
mod perf_bench {
    use super::*;
    use crate::database::init_schema;
    use crate::db::ConnectionDb;
    use rusqlite::{params, Connection};
    use std::time::Instant;

    #[test]
    #[ignore]
    fn perf_bench_library_load() {
        let mut conn = Connection::open_in_memory().expect("open in-memory db");
        init_schema(&conn).expect("init schema");

        let date_added = Utc::now().to_rfc3339();
        {
            let tx = conn.transaction().expect("transaction");
            {
                let mut stmt = tx
                    .prepare(
                        "INSERT INTO games (id, name, exe_path, exe_name, date_added) VALUES (?1, ?2, ?3, ?4, ?5)",
                    )
                    .expect("prepare insert");
                for i in 0..5000 {
                    let id = format!("bench-{i}");
                    let name = format!("Game {i:04}");
                    let exe_name = format!("game-{i}.exe");
                    let exe_path = format!("C:/Games/{id}/{exe_name}");
                    stmt.execute(params![id, name, exe_path, exe_name, date_added])
                        .expect("insert game");
                }
            }
            tx.commit().expect("commit");
        }

        let db = ConnectionDb::new(conn);
        let start = Instant::now();
        let games = get_all_games(&db).expect("get all games");
        let serialized = serde_json::to_vec(&games).expect("serialize games");
        let elapsed = start.elapsed();

        println!(
            "perf: library_load rows={} bytes={} duration_ms={}",
            games.len(),
            serialized.len(),
            elapsed.as_millis()
        );
    }
}
