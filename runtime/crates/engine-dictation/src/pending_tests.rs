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
    assert!(
        wav_path(td.path(), &uuid).exists(),
        "WAV должен существовать"
    );
    assert!(
        meta_path(td.path(), &uuid).exists(),
        "JSON должен существовать"
    );
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
    let r = drop_item(td.path(), &Uuid::new_v4().to_string());
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
    let r = read_wav(td.path(), &Uuid::new_v4().to_string());
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
    assert_eq!(
        tmp_count, 0,
        "после успешного enqueue .tmp не должно остаться"
    );
}

#[test]
fn gc_removes_items_older_than_max_age() {
    let td = TempDir::new().expect("tempdir");
    let old = enqueue(td.path(), b"old", 1.0, opts()).unwrap();
    let fresh = enqueue(td.path(), b"fresh", 1.0, opts()).unwrap();

    // Делаем "old" реально старым — переписываем JSON с past created_at.
    let mut item: PendingItem =
        serde_json::from_slice(&fs::read(meta_path(td.path(), &old)).unwrap()).unwrap();
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

#[test]
fn drop_item_rejects_traversal_uuid() {
    let td = TempDir::new().expect("tempdir");
    // "../../victim" из pending/ выходит в <dataDir> — файл вне очереди.
    let victim = td.path().join("victim.wav");
    fs::write(&victim, b"keep me").unwrap();
    let victim_meta = td.path().join("victim.json");
    fs::write(&victim_meta, b"keep me").unwrap();

    let err = drop_item(td.path(), "../../victim").unwrap_err();
    assert!(matches!(err, PendingError::InvalidUuid(_)));
    assert!(victim.exists(), "traversal uuid не должен удалять wav");
    assert!(
        victim_meta.exists(),
        "traversal uuid не должен удалять json"
    );
}

#[test]
fn read_wav_and_bump_attempt_reject_traversal_uuid() {
    let td = TempDir::new().expect("tempdir");
    let victim = td.path().join("victim.wav");
    fs::write(&victim, b"secret").unwrap();
    assert!(matches!(
        read_wav(td.path(), "../../victim"),
        Err(PendingError::InvalidUuid(_))
    ));
    assert!(matches!(
        bump_attempt(td.path(), "../dictation/victim", "x"),
        Err(PendingError::InvalidUuid(_))
    ));
}

#[test]
fn delivered_items_stay_in_history_but_leave_the_queue() {
    let td = TempDir::new().unwrap();
    let uuid = enqueue(
        td.path(),
        b"wav",
        1.0,
        EnqueueOpts {
            language: "ru".into(),
            prompt: String::new(),
            inject_mode: "auto_paste".into(),
            model: "parakeet".into(),
            prev_hwnd: None,
        },
    )
    .unwrap();

    mark_delivered(td.path(), &uuid, "привет").unwrap();
    // История: запись осталась со статусом и текстом, WAV удалён.
    let items = list(td.path()).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].status.as_deref(), Some("delivered"));
    assert_eq!(items[0].transcript.as_deref(), Some("привет"));
    assert!(!wav_path(td.path(), &uuid).exists());
    // А из unresolved-очереди она ушла — retry её не трогает.
    assert!(list_unresolved(td.path()).unwrap().is_empty());
    // drop_all сносит и историю.
    assert_eq!(drop_all(td.path()).unwrap(), 1);
    assert!(list(td.path()).unwrap().is_empty());
}
