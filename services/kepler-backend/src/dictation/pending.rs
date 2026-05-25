// Persistent очередь для аудио, ждущего транскрипции.
//
// Disk-first: WAV пишется в `<dataDir>/dictation/pending/<uuid>.wav` ДО
// HTTP-запроса к Groq. Метаданные — рядом в `<uuid>.json`. Атомарность
// через `tmp → rename`. Это гарантирует:
//   - crash backend / kill process / OS reboot mid-transcribe → аудио
//     остаётся на диске, retry возможен после рестарта.
//   - Network drop → backend помечает attempts++, оставляет файлы,
//     юзер видит pending в Settings → Диктация и может Retry.
//
// GC: на app start удалить items старше N дней ИЛИ если их больше M штук
// (LRU, по created_at). См. forbidden.md § Dictation.

#![allow(dead_code)] // wired в host.rs в task #11

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

const SUBDIR: &str = "dictation/pending";

#[derive(Debug, Error)]
pub(crate) enum PendingError {
    #[error("io: {0}")]
    Io(#[from] io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("uuid '{0}' не найден в очереди")]
    NotFound(String),
}

/// Параметры транскрипции, сохраняемые рядом с WAV. При retry эти параметры
/// дотягиваются из JSON — гарантирует что повтор использует тот же контекст
/// (язык, prompt, inject mode) что и исходный submit.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct EnqueueOpts {
    pub language: String,
    pub prompt: String,
    pub inject_mode: String, // "auto_paste" | "clipboard_only"
    pub model: String,
    pub prev_hwnd: Option<isize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PendingItem {
    pub uuid: String,
    pub created_at: DateTime<Utc>,
    pub attempts: u32,
    pub last_error: Option<String>,
    pub opts: EnqueueOpts,
    /// Длительность аудио в секундах (для UI отображения).
    pub duration_sec: f32,
    /// Размер WAV в байтах (для GC по размеру + sanity check).
    pub wav_bytes: u64,
}

fn pending_dir(data_dir: &Path) -> PathBuf {
    data_dir.join(SUBDIR)
}

fn wav_path(data_dir: &Path, uuid: &str) -> PathBuf {
    pending_dir(data_dir).join(format!("{uuid}.wav"))
}

fn meta_path(data_dir: &Path, uuid: &str) -> PathBuf {
    pending_dir(data_dir).join(format!("{uuid}.json"))
}

/// Атомарная запись: `path.tmp` → fsync → rename. Гарантирует что reader
/// никогда не увидит partial-файл (rename атомарен в POSIX и NTFS).
fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp = path.with_extension(format!(
        "{}.tmp",
        path.extension().and_then(|e| e.to_str()).unwrap_or("")
    ));
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path)?;
    Ok(())
}

/// Сохраняет WAV + метаданные. Возвращает сгенерированный UUID v4.
pub(crate) fn enqueue(
    data_dir: &Path,
    wav: &[u8],
    duration_sec: f32,
    opts: EnqueueOpts,
) -> Result<String, PendingError> {
    fs::create_dir_all(pending_dir(data_dir))?;
    let uuid = Uuid::new_v4().to_string();
    atomic_write(&wav_path(data_dir, &uuid), wav)?;
    let item = PendingItem {
        uuid: uuid.clone(),
        created_at: Utc::now(),
        attempts: 0,
        last_error: None,
        opts,
        duration_sec,
        wav_bytes: wav.len() as u64,
    };
    let json = serde_json::to_vec_pretty(&item)?;
    atomic_write(&meta_path(data_dir, &uuid), &json)?;
    Ok(uuid)
}

/// Все pending items, отсортированные по `created_at` ASC (старые первыми —
/// retry-all обрабатывает в порядке поступления).
pub(crate) fn list(data_dir: &Path) -> Result<Vec<PendingItem>, PendingError> {
    let dir = pending_dir(data_dir);
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut items = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        // Игнорируем .tmp / битые JSON — pending-очередь best-effort,
        // битый item можно перечитать с диска или удалить вручную.
        let bytes = match fs::read(&path) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let item: PendingItem = match serde_json::from_slice(&bytes) {
            Ok(i) => i,
            Err(_) => continue,
        };
        // WAV должен существовать — иначе orphan-метаданные, чистим.
        if !wav_path(data_dir, &item.uuid).exists() {
            let _ = fs::remove_file(&path);
            continue;
        }
        items.push(item);
    }
    items.sort_by_key(|i| i.created_at);
    Ok(items)
}

/// Удаляет WAV + JSON. Для использования после успешного transcribe+inject
/// или явного юзером discard.
pub(crate) fn drop_item(data_dir: &Path, uuid: &str) -> Result<(), PendingError> {
    let wav = wav_path(data_dir, uuid);
    let meta = meta_path(data_dir, uuid);
    if !wav.exists() && !meta.exists() {
        return Err(PendingError::NotFound(uuid.into()));
    }
    // best-effort: оба удаляем, ошибка одного не блокирует другого.
    let _ = fs::remove_file(&wav);
    let _ = fs::remove_file(&meta);
    Ok(())
}

/// Инкрементит `attempts`, обновляет `last_error`, перезаписывает JSON.
pub(crate) fn bump_attempt(
    data_dir: &Path,
    uuid: &str,
    err: &str,
) -> Result<PendingItem, PendingError> {
    let path = meta_path(data_dir, uuid);
    if !path.exists() {
        return Err(PendingError::NotFound(uuid.into()));
    }
    let bytes = fs::read(&path)?;
    let mut item: PendingItem = serde_json::from_slice(&bytes)?;
    item.attempts = item.attempts.saturating_add(1);
    item.last_error = Some(err.to_string());
    let json = serde_json::to_vec_pretty(&item)?;
    atomic_write(&path, &json)?;
    Ok(item)
}

/// Читает сырые WAV bytes для retry.
pub(crate) fn read_wav(data_dir: &Path, uuid: &str) -> Result<Vec<u8>, PendingError> {
    let path = wav_path(data_dir, uuid);
    if !path.exists() {
        return Err(PendingError::NotFound(uuid.into()));
    }
    Ok(fs::read(&path)?)
}

/// Garbage collection. Удаляет items:
///   1. старше `max_age` (по created_at).
///   2. сверх `max_items` (оставляя `max_items` самых свежих).
///
/// Возвращает количество удалённых.
pub(crate) fn gc(
    data_dir: &Path,
    max_items: usize,
    max_age: ChronoDuration,
) -> Result<u32, PendingError> {
    let items = list(data_dir)?;
    let now = Utc::now();
    let cutoff = now - max_age;

    // 1. По возрасту
    let mut removed = 0u32;
    let mut survivors = Vec::with_capacity(items.len());
    for item in items {
        if item.created_at < cutoff {
            drop_item(data_dir, &item.uuid)?;
            removed += 1;
        } else {
            survivors.push(item);
        }
    }

    // 2. По количеству — оставить max_items самых свежих
    if survivors.len() > max_items {
        // survivors отсортированы ASC по created_at — старые в начале
        let to_remove = survivors.len() - max_items;
        for item in survivors.iter().take(to_remove) {
            drop_item(data_dir, &item.uuid)?;
            removed += 1;
        }
    }
    Ok(removed)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn opts() -> EnqueueOpts {
        EnqueueOpts {
            language: "ru".into(),
            prompt: String::new(),
            inject_mode: "auto_paste".into(),
            model: "whisper-large-v3".into(),
            prev_hwnd: None,
        }
    }

    #[test]
    fn enqueue_creates_wav_and_json() {
        let td = TempDir::new().expect("tempdir");
        let uuid = enqueue(td.path(), b"fake-wav-bytes", 2.5, opts()).expect("enqueue");
        assert!(wav_path(td.path(), &uuid).exists(), "WAV должен существовать");
        assert!(meta_path(td.path(), &uuid).exists(), "JSON должен существовать");
    }

    #[test]
    fn enqueue_returns_unique_uuids() {
        let td = TempDir::new().expect("tempdir");
        let a = enqueue(td.path(), b"a", 1.0, opts()).unwrap();
        let b = enqueue(td.path(), b"b", 1.0, opts()).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn list_returns_sorted_by_created_at() {
        let td = TempDir::new().expect("tempdir");
        let a = enqueue(td.path(), b"a", 1.0, opts()).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let b = enqueue(td.path(), b"b", 1.0, opts()).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let c = enqueue(td.path(), b"c", 1.0, opts()).unwrap();
        let items = list(td.path()).expect("list");
        let uuids: Vec<_> = items.iter().map(|i| i.uuid.clone()).collect();
        assert_eq!(uuids, vec![a, b, c], "ASC по created_at");
    }

    #[test]
    fn list_empty_when_no_dir() {
        let td = TempDir::new().expect("tempdir");
        let items = list(td.path()).expect("list on empty");
        assert!(items.is_empty());
    }

    #[test]
    fn list_preserves_opts_fields() {
        let td = TempDir::new().expect("tempdir");
        let mut o = opts();
        o.language = "en".into();
        o.prompt = "custom hint".into();
        o.prev_hwnd = Some(12345);
        let uuid = enqueue(td.path(), b"x", 3.25, o.clone()).unwrap();
        let items = list(td.path()).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].uuid, uuid);
        assert_eq!(items[0].opts, o);
        assert_eq!(items[0].duration_sec, 3.25);
        assert_eq!(items[0].wav_bytes, 1);
    }

    #[test]
    fn drop_removes_both_files() {
        let td = TempDir::new().expect("tempdir");
        let uuid = enqueue(td.path(), b"a", 1.0, opts()).unwrap();
        drop_item(td.path(), &uuid).expect("drop");
        assert!(!wav_path(td.path(), &uuid).exists());
        assert!(!meta_path(td.path(), &uuid).exists());
    }

    #[test]
    fn drop_unknown_returns_not_found() {
        let td = TempDir::new().expect("tempdir");
        let r = drop_item(td.path(), "does-not-exist");
        assert!(matches!(r, Err(PendingError::NotFound(_))));
    }

    #[test]
    fn bump_attempt_increments_and_sets_error() {
        let td = TempDir::new().expect("tempdir");
        let uuid = enqueue(td.path(), b"a", 1.0, opts()).unwrap();
        bump_attempt(td.path(), &uuid, "first fail").unwrap();
        bump_attempt(td.path(), &uuid, "second fail").unwrap();
        let items = list(td.path()).unwrap();
        assert_eq!(items[0].attempts, 2);
        assert_eq!(items[0].last_error, Some("second fail".into()));
    }

    #[test]
    fn read_wav_returns_bytes() {
        let td = TempDir::new().expect("tempdir");
        let original = b"\x00\x01\x02 wav data";
        let uuid = enqueue(td.path(), original, 1.0, opts()).unwrap();
        let read = read_wav(td.path(), &uuid).unwrap();
        assert_eq!(read, original);
    }

    #[test]
    fn read_wav_unknown_returns_not_found() {
        let td = TempDir::new().expect("tempdir");
        let r = read_wav(td.path(), "nope");
        assert!(matches!(r, Err(PendingError::NotFound(_))));
    }

    #[test]
    fn atomic_write_leaves_no_tmp_on_success() {
        let td = TempDir::new().expect("tempdir");
        let _ = enqueue(td.path(), b"a", 1.0, opts()).unwrap();
        let dir = pending_dir(td.path());
        let tmp_count = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .and_then(|s| s.to_str())
                    .map(|s| s.ends_with(".tmp"))
                    .unwrap_or(false)
            })
            .count();
        assert_eq!(tmp_count, 0, "после успешного enqueue .tmp не должно остаться");
    }

    #[test]
    fn gc_removes_items_older_than_max_age() {
        let td = TempDir::new().expect("tempdir");
        let old = enqueue(td.path(), b"old", 1.0, opts()).unwrap();
        let fresh = enqueue(td.path(), b"fresh", 1.0, opts()).unwrap();

        // Делаем "old" реально старым — переписываем JSON с past created_at.
        let mut item: PendingItem = serde_json::from_slice(
            &fs::read(meta_path(td.path(), &old)).unwrap(),
        )
        .unwrap();
        item.created_at = Utc::now() - ChronoDuration::days(10);
        fs::write(
            meta_path(td.path(), &old),
            serde_json::to_vec_pretty(&item).unwrap(),
        )
        .unwrap();

        let removed = gc(td.path(), 100, ChronoDuration::days(7)).unwrap();
        assert_eq!(removed, 1);
        let remaining = list(td.path()).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].uuid, fresh);
    }

    #[test]
    fn gc_caps_by_max_items_keeping_freshest() {
        let td = TempDir::new().expect("tempdir");
        let mut uuids = Vec::new();
        for _ in 0..5 {
            let u = enqueue(td.path(), b"x", 1.0, opts()).unwrap();
            uuids.push(u);
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        // max=3 → удалить 2 самых старых
        let removed = gc(td.path(), 3, ChronoDuration::days(365)).unwrap();
        assert_eq!(removed, 2);
        let remaining = list(td.path()).unwrap();
        assert_eq!(remaining.len(), 3);
        // Survivors — последние 3 (uuids[2..])
        let surviving: Vec<_> = remaining.iter().map(|i| i.uuid.clone()).collect();
        assert_eq!(surviving, uuids[2..].to_vec());
    }

    #[test]
    fn gc_noop_when_within_limits() {
        let td = TempDir::new().expect("tempdir");
        enqueue(td.path(), b"x", 1.0, opts()).unwrap();
        enqueue(td.path(), b"y", 1.0, opts()).unwrap();
        let removed = gc(td.path(), 100, ChronoDuration::days(7)).unwrap();
        assert_eq!(removed, 0);
        assert_eq!(list(td.path()).unwrap().len(), 2);
    }

    #[test]
    fn list_skips_orphan_meta_without_wav() {
        let td = TempDir::new().expect("tempdir");
        let uuid = enqueue(td.path(), b"x", 1.0, opts()).unwrap();
        // Удаляем только WAV — JSON остаётся orphan'ом.
        fs::remove_file(wav_path(td.path(), &uuid)).unwrap();
        let items = list(td.path()).unwrap();
        assert!(items.is_empty(), "orphan JSON должен быть скрыт");
        // И автоматически удалён списком — следующий list тоже видит пусто.
        assert!(!meta_path(td.path(), &uuid).exists());
    }

    #[test]
    fn list_skips_malformed_json() {
        let td = TempDir::new().expect("tempdir");
        let dir = pending_dir(td.path());
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("garbage.json"), b"{not valid json").unwrap();
        let items = list(td.path()).unwrap();
        assert!(items.is_empty(), "битый JSON не должен паниковать");
    }
}
