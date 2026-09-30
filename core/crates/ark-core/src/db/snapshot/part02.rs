/// KOS-83: при двойном провале (post-verify + rollback) pre-restore snapshot —
/// последний путь к данным пользователя, его нельзя удалять. Переименовываем
/// hidden `.restore-rollback-*` в обычный basename, чтобы копия попадала в
/// `db_backup_list` и восстанавливалась штатным `db_backup_restore`.
/// Возвращает текст для RPC-ошибки: где искать сохранённую копию.
fn preserve_rollback_snapshot(db_path: &str, rollback: &Path, nonce: &str) -> String {
    if !rollback.exists() {
        return "pre-restore snapshot is missing; manual DB recovery required".to_string();
    }
    let kept_name = format!("ark.db.pre-restore-failed-{nonce}.db");
    let kept = backups_dir(db_path).join(&kept_name);
    match fs::rename(rollback, &kept) {
        Ok(()) => format!(
            "pre-restore snapshot preserved as '{kept_name}' in backups dir; \
             restore it via db_backup_restore to recover prior data"
        ),
        Err(_) => format!(
            "pre-restore snapshot preserved at '{}'; manual recovery required",
            rollback.display()
        ),
    }
}

/// `db_backup_restore`: атомарно восстанавливает live DB из snapshot'а.
/// Вызывается под глобальным DB mutex из RPC handler'а — параллельных
/// reader/writer нет, backup-поток остановлен `BACKUP_GATE`.
pub fn restore_snapshot(
    conn: &mut Connection,
    db_path: &str,
    id: &str,
) -> Result<RestoreReport, String> {
    restore_snapshot_impl(conn, db_path, id, &default_post_verify, &default_rollback_restore)
}

fn restore_snapshot_impl(
    conn: &mut Connection,
    db_path: &str,
    id: &str,
    post_verify: &PostVerify,
    rollback_restore: &RollbackRestore,
) -> Result<RestoreReport, String> {
    // 1. Resolve + stage (no-follow, содержимое frozen до apply).
    let staging = stage_snapshot(db_path, id)?;

    let rollback_nonce = format!("{:016x}", rand::random::<u64>());
    let rollback = backups_dir(db_path).join(format!(".restore-rollback-{rollback_nonce}.db"));
    let cleanup = |staging: &Path, rollback: &Path| {
        let _ = fs::remove_file(staging);
        let _ = fs::remove_file(rollback);
    };

    // 2. Validate staging: integrity + schema match против live.
    let validated = (|| -> Result<Vec<String>, String> {
        let src = Connection::open_with_flags(&staging, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| format!("snapshot open failed: {e}"))?;
        check_integrity(&src)?;
        schema_fingerprint(&src)
    })();
    let snapshot_fp = match validated {
        Ok(fp) => fp,
        Err(e) => {
            cleanup(&staging, &rollback);
            return Err(e);
        }
    };
    let live_fp = schema_fingerprint(conn)?;
    if snapshot_fp != live_fp {
        cleanup(&staging, &rollback);
        return Err("snapshot schema mismatch with live ARK DB".to_string());
    }

    // 3. Pre-restore rollback snapshot live DB (Online Backup в отдельный
    //    файл — консистентно при idle conn под mutex'ом).
    if let Err(e) = conn.backup(rusqlite::DatabaseName::Main, &rollback, None) {
        cleanup(&staging, &rollback);
        return Err(format!("pre-restore rollback snapshot failed: {e}"));
    }

    // 4. Apply: Online Backup API пишет destination транзакционно.
    if let Err(e) = conn.restore(
        rusqlite::DatabaseName::Main,
        &staging,
        None::<fn(rusqlite::backup::Progress)>,
    ) {
        cleanup(&staging, &rollback);
        return Err(format!("restore apply failed: {e}"));
    }

    // 5. Post-restore verification; провал → откат из rollback snapshot'а.
    if let Err(verify_err) = post_verify(conn, &snapshot_fp) {
        let rollback_result = rollback_restore(conn, &rollback);
        // Staging — временная копия нетронутого snapshot'а из backups/,
        // чистим всегда. Rollback-файл удаляем только при успешном откате:
        // при двойном провале это последний путь к pre-restore данным —
        // сохраняем под видимым для db_backup_list именем (KOS-83).
        let _ = fs::remove_file(&staging);
        return Err(match rollback_result {
            Ok(()) => {
                let _ = fs::remove_file(&rollback);
                format!(
                    "post-restore verification failed ({verify_err}); live DB rolled back"
                )
            }
            Err(rollback_err) => format!(
                "post-restore verification failed ({verify_err}); rollback failed: \
                 {rollback_err}; {}",
                preserve_rollback_snapshot(db_path, &rollback, &rollback_nonce)
            ),
        });
    }

    // 6. Typed result для Cortex (per-entity counts для UI confirmation).
    let report = (|| -> Result<RestoreReport, String> {
        let objects = conn
            .query_row("SELECT COUNT(*) FROM objects", [], |row| row.get(0))
            .map_err(|e| e.to_string())?;
        let links = conn
            .query_row("SELECT COUNT(*) FROM object_links", [], |row| row.get(0))
            .map_err(|e| e.to_string())?;
        Ok(RestoreReport {
            id: id.to_string(),
            restored: true,
            objects,
            links,
        })
    })();
    cleanup(&staging, &rollback);
    report
}
