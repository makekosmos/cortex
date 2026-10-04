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

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::data_dir::temp_sweep::{self, LEFTOVER_GRACE};

const SUBDIR: &str = "dictation/pending";

#[derive(Debug, Error)]
pub(crate) enum PendingError {
    #[error("io: {0}")]
    Io(#[from] io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("uuid '{0}' не найден в очереди")]
    NotFound(String),
    #[error("невалидный uuid '{0}'")]
    InvalidUuid(String),
}

/// Pending filenames строятся как `{uuid}.wav`/`.json` — без проверки строка
/// вроде "../../x" выходит за `dictation/pending` (path traversal из WS-опа
/// `dictation.discard`). Принимаем только канонический UUID v4, как у enqueue.
fn require_uuid(uuid: &str) -> Result<(), PendingError> {
    if Uuid::parse_str(uuid).is_ok() {
        Ok(())
    } else {
        Err(PendingError::InvalidUuid(uuid.into()))
    }
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
    // KOS-301: crash между `File::create` и `rename` в atomic_write оставлял
    // `{uuid}.wav.tmp`/`{uuid}.json.tmp` навсегда — чистим старше grace.
    temp_sweep::sweep(&dir, LEFTOVER_GRACE, |name, is_dir| {
        !is_dir && name.ends_with(".tmp")
    });
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
    require_uuid(uuid)?;
    let wav = wav_path(data_dir, uuid);
    let meta = meta_path(data_dir, uuid);
    if !wav.exists() && !meta.exists() {
        return Err(PendingError::NotFound(uuid.into()));
    }
    let mut removed = false;
    let mut first_error = None;
    for path in [&wav, &meta] {
        match fs::remove_file(path) {
            Ok(()) => removed = true,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) if first_error.is_none() => first_error = Some(error),
            Err(_) => {}
        }
    }
    if let Some(error) = first_error {
        return Err(PendingError::Io(error));
    }
    if removed {
        Ok(())
    } else {
        Err(PendingError::NotFound(uuid.into()))
    }
}

/// Инкрементит `attempts`, обновляет `last_error`, перезаписывает JSON.
pub(crate) fn bump_attempt(
    data_dir: &Path,
    uuid: &str,
    err: &str,
) -> Result<PendingItem, PendingError> {
    require_uuid(uuid)?;
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
    require_uuid(uuid)?;
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
#[cfg(test)]
#[path = "pending_tests.rs"]
mod tests;
