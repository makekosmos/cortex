use std::fs;
use std::path::Path;

use rusqlite::{Connection, OpenFlags};

use super::{
    default_post_verify, default_rollback_restore, recovery, restore_journal, schema_fingerprint,
    stage_snapshot, PostVerify, RestoreReport, RollbackRestore,
};

/// `db_backup_restore`: атомарно восстанавливает live DB из snapshot'а.
/// Вызывается под глобальным DB mutex из RPC handler'а.
/// Параллельных запросов нет; backup остановлен через `BACKUP_GATE`.
pub fn restore_snapshot(
    conn: &mut Connection,
    db_path: &str,
    id: &str,
) -> Result<RestoreReport, String> {
    restore_snapshot_impl(
        conn,
        db_path,
        id,
        &default_post_verify,
        &default_rollback_restore,
    )
}

fn restore_snapshot_impl(
    conn: &mut Connection,
    db_path: &str,
    id: &str,
    post_verify: &PostVerify<'_>,
    rollback_restore: &RollbackRestore<'_>,
) -> Result<RestoreReport, String> {
    // 1. Resolve + stage (no-follow, содержимое frozen до apply).
    let staging = stage_snapshot(db_path, id)?;
    let cleanup_staging = |staging: &Path| {
        let _ = fs::remove_file(staging);
    };

    // 2. Validate staging: integrity + schema match против live.
    let validated = (|| -> Result<(Vec<String>, i64), String> {
        let src = Connection::open_with_flags(&staging, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| format!("snapshot open failed: {e}"))?;
        crate::open::check_integrity(&src)?;
        Ok((
            schema_fingerprint(&src)?,
            crate::committed_feed::watermark(&src)?,
        ))
    })();
    let (snapshot_fp, snapshot_feed_watermark) = match validated {
        Ok(snapshot) => snapshot,
        Err(e) => {
            cleanup_staging(&staging);
            return Err(e);
        }
    };
    let live_fp = schema_fingerprint(conn)?;
    if snapshot_fp != live_fp {
        cleanup_staging(&staging);
        return Err("snapshot schema mismatch with live ARK DB".to_string());
    }

    // Restoring an older sync cursor can make a peer-observed feed position
    // available for reuse. The feed stores locators, not entity payloads, so
    // it cannot reconstruct post-snapshot changes. Refuse before changing the
    // live DB; a recovery/resnapshot protocol can safely relax this boundary.
    let live_feed_watermark = match crate::committed_feed::watermark(conn) {
        Ok(watermark) => watermark,
        Err(error) => {
            cleanup_staging(&staging);
            return Err(format!(
                "live committed feed watermark unavailable: {error}"
            ));
        }
    };
    if snapshot_feed_watermark < live_feed_watermark {
        cleanup_staging(&staging);
        return Err(format!(
            concat!(
                "restore refused: stale snapshot predates committed sync history ",
                "(snapshot feed {}, live feed {})"
            ),
            snapshot_feed_watermark, live_feed_watermark
        ));
    }

    if let Err(e) = recovery::preserve_live_device_identity(conn, &staging) {
        cleanup_staging(&staging);
        return Err(format!("restore identity recovery failed: {e}"));
    }
    let journal = match restore_journal::RestoreJournal::begin(conn, db_path, &snapshot_fp) {
        Ok(journal) => journal,
        Err(error) => {
            cleanup_staging(&staging);
            return Err(format!("pre-restore recovery journal failed: {error}"));
        }
    };
    let rollback = journal.capsule_path().to_path_buf();

    // 4. Apply: Online Backup API пишет destination транзакционно.
    if let Err(e) = conn.restore(
        rusqlite::DatabaseName::Main,
        &staging,
        None::<fn(rusqlite::backup::Progress)>,
    ) {
        let rollback_result = rollback_restore(conn, &rollback);
        cleanup_staging(&staging);
        return match rollback_result {
            Ok(()) => match journal.finish() {
                Ok(()) => Err(format!("restore apply failed ({e}); live DB rolled back")),
                Err(recovery_error) => Err(format!(
                    "restore recovery required: apply {e}; journal: {recovery_error}"
                )),
            },
            Err(rollback_error) => {
                let preserved = journal.preserve_visible_backup(db_path);
                Err(match preserved {
                    Ok(name) => format!(
                        "restore recovery required: apply failed ({e}); \
                         rollback failed ({rollback_error}); \
                         pre-restore snapshot preserved as '{name}'"
                    ),
                    Err(preserve_error) => format!(
                        "restore recovery required: apply failed ({e}); \
                         rollback failed ({rollback_error}); backup at '{}'; \
                         visibility failed: {preserve_error}",
                        rollback.display()
                    ),
                })
            }
        };
    }

    // 5. Post-restore verification; провал → откат из rollback snapshot'а.
    if let Err(verify_err) = post_verify(conn, &snapshot_fp) {
        let rollback_result = rollback_restore(conn, &rollback);
        cleanup_staging(&staging);
        return Err(match rollback_result {
            Ok(()) => match journal.finish() {
                Ok(()) => {
                    format!("post-restore verification failed ({verify_err}); live DB rolled back")
                }
                Err(recovery_error) => format!(
                    "restore recovery required: verify {verify_err}; journal: {recovery_error}"
                ),
            },
            Err(rollback_err) => match journal.preserve_visible_backup(db_path) {
                Ok(name) => format!(
                    "restore recovery required: verification failed ({verify_err}); \
                     rollback failed: {rollback_err}; \
                     pre-restore snapshot preserved as '{name}'"
                ),
                Err(preserve_error) => format!(
                    "restore recovery required: verification failed ({verify_err}); \
                     rollback failed: {rollback_err}; backup at '{}'; \
                     visibility failed: {preserve_error}",
                    rollback.display()
                ),
            },
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
    let report = match report {
        Ok(report) => report,
        Err(error) => {
            let rollback_result = rollback_restore(conn, &rollback);
            cleanup_staging(&staging);
            return match rollback_result {
                Ok(()) => match journal.finish() {
                    Ok(()) => Err(format!(
                        "restore report failed ({error}); live DB rolled back"
                    )),
                    Err(recovery_error) => Err(format!(
                        "restore recovery required: report {error}; journal: {recovery_error}"
                    )),
                },
                Err(rollback_error) => match journal.preserve_visible_backup(db_path) {
                    Ok(name) => Err(format!(
                        "restore recovery required: report failed ({error}); \
                         rollback failed ({rollback_error}); \
                         pre-restore snapshot preserved as '{name}'"
                    )),
                    Err(preserve_error) => Err(format!(
                        "restore recovery required: report failed ({error}); \
                         rollback failed ({rollback_error}); backup at '{}'; \
                         visibility failed: {preserve_error}",
                        rollback.display()
                    )),
                },
            };
        }
    };
    journal
        .finish()
        .map_err(|error| format!("restore recovery required: {error}"))?;
    cleanup_staging(&staging);
    Ok(report)
}

#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
pub fn restore_snapshot_with_hooks(
    conn: &mut Connection,
    db_path: &str,
    id: &str,
    post_verify: impl Fn(&Connection, &[String]) -> Result<(), String>,
    rollback_restore: impl Fn(&mut Connection, &Path) -> Result<(), String>,
) -> Result<RestoreReport, String> {
    restore_snapshot_impl(conn, db_path, id, &post_verify, &rollback_restore)
}
