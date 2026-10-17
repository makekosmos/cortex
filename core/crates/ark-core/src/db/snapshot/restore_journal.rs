use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{backups_dir, check_integrity, open_no_follow, sync_file};

const INTENT_SUFFIX: &str = ".restore-intent.json";
const INTENT_LIMIT: u64 = 4096;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RestoreIntent {
    version: u32,
    operation_id: String,
    capsule_sha256: String,
}

pub(super) struct RestoreJournal {
    directory: PathBuf,
    intent_path: PathBuf,
    capsule_path: PathBuf,
    operation_id: String,
}

impl RestoreJournal {
    pub(super) fn begin(
        connection: &Connection,
        db_path: &str,
        expected_schema: &[String],
    ) -> Result<Self, String> {
        let directory = recovery_directory(db_path)?;
        let intent_path = intent_path(db_path, &directory)?;
        if intent_path.exists() {
            return Err("restore recovery is required before another restore".into());
        }

        let operation_id = format!("{:016x}", rand::random::<u64>());
        let prefix = database_prefix(db_path)?;
        let capsule_path = directory.join(format!(".{prefix}.restore-live-{operation_id}.db"));
        let staged_intent = directory.join(format!(".{prefix}.restore-intent-{operation_id}.tmp"));
        let result = (|| -> Result<(), String> {
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&capsule_path)
                .map_err(|error| format!("restore recovery capsule create failed: {error}"))?;
            connection
                .backup(rusqlite::DatabaseName::Main, &capsule_path, None)
                .map_err(|error| format!("restore recovery capsule backup failed: {error}"))?;
            let capsule =
                Connection::open_with_flags(&capsule_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
                    .map_err(|error| format!("restore recovery capsule open failed: {error}"))?;
            check_integrity(&capsule)?;
            if super::schema_fingerprint(&capsule)? != expected_schema {
                return Err("restore recovery capsule schema changed during backup".into());
            }
            drop(capsule);
            sync_file(&capsule_path)
                .map_err(|error| format!("restore recovery capsule sync failed: {error}"))?;
            sync_directory(&directory).map_err(|error| {
                format!("restore recovery capsule publish sync failed: {error}")
            })?;

            let intent = RestoreIntent {
                version: 1,
                operation_id: operation_id.clone(),
                capsule_sha256: digest_file(&capsule_path)?,
            };
            let bytes = serde_json::to_vec(&intent)
                .map_err(|error| format!("restore recovery intent encode failed: {error}"))?;
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&staged_intent)
                .map_err(|error| format!("restore recovery intent stage failed: {error}"))?;
            file.write_all(&bytes)
                .and_then(|()| file.sync_all())
                .map_err(|error| format!("restore recovery intent sync failed: {error}"))?;
            fs::hard_link(&staged_intent, &intent_path)
                .map_err(|error| format!("restore recovery intent publish failed: {error}"))?;
            sync_directory(&directory)
                .map_err(|error| format!("restore recovery directory sync failed: {error}"))?;
            Ok(())
        })();
        let _ = fs::remove_file(&staged_intent);
        if let Err(error) = result {
            if !intent_path.exists() {
                remove_capsule(&capsule_path);
            }
            return Err(error);
        }
        Ok(Self {
            directory,
            intent_path,
            capsule_path,
            operation_id,
        })
    }

    pub(super) fn capsule_path(&self) -> &Path {
        &self.capsule_path
    }

    pub(super) fn preserve_visible_backup(&self, db_path: &str) -> Result<String, String> {
        let name = format!("ark.db.pre-restore-failed-{}.db", self.operation_id);
        let visible = backups_dir(db_path).join(&name);
        fs::hard_link(&self.capsule_path, &visible)
            .map_err(|error| format!("pre-restore snapshot visibility failed: {error}"))?;
        sync_directory(&backups_dir(db_path))
            .map_err(|error| format!("pre-restore snapshot directory sync failed: {error}"))?;
        Ok(name)
    }

    pub(super) fn finish(self) -> Result<(), String> {
        fs::remove_file(&self.intent_path)
            .map_err(|error| format!("restore recovery intent removal failed: {error}"))?;
        sync_directory(&self.directory)
            .map_err(|error| format!("restore recovery completion sync failed: {error}"))?;
        remove_capsule(&self.capsule_path);
        Ok(())
    }
}

pub(crate) fn recover_interrupted_restore(db_path: &str) -> Result<(), String> {
    let directory = recovery_directory(db_path)?;
    let intent_path = intent_path(db_path, &directory)?;
    if !intent_path.exists() {
        return Ok(());
    }
    let mut file = open_no_follow(&intent_path)?;
    let length = file
        .metadata()
        .map_err(|error| format!("restore recovery intent metadata failed: {error}"))?
        .len();
    if length > INTENT_LIMIT {
        return Err("restore recovery intent exceeds size limit".into());
    }
    let mut bytes = Vec::with_capacity(length as usize);
    file.read_to_end(&mut bytes)
        .map_err(|error| format!("restore recovery intent read failed: {error}"))?;
    let intent: RestoreIntent = serde_json::from_slice(&bytes)
        .map_err(|error| format!("restore recovery intent is invalid: {error}"))?;
    if intent.version != 1
        || intent.operation_id.len() != 16
        || !intent
            .operation_id
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || intent.capsule_sha256.len() != 64
        || !intent
            .capsule_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("restore recovery intent fields are invalid".into());
    }
    let prefix = database_prefix(db_path)?;
    let capsule_path = directory.join(format!(".{prefix}.restore-live-{}.db", intent.operation_id));
    if digest_file(&capsule_path)? != intent.capsule_sha256 {
        return Err("restore recovery capsule digest mismatch".into());
    }
    let capsule = Connection::open_with_flags(&capsule_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|error| format!("restore recovery capsule open failed: {error}"))?;
    check_integrity(&capsule)?;
    drop(capsule);

    let mut live = Connection::open(db_path)
        .map_err(|error| format!("restore recovery live DB open failed: {error}"))?;
    live.restore(
        rusqlite::DatabaseName::Main,
        &capsule_path,
        None::<fn(rusqlite::backup::Progress)>,
    )
    .map_err(|error| format!("restore recovery rollback failed: {error}"))?;
    check_integrity(&live)?;
    drop(live);

    fs::remove_file(&intent_path)
        .map_err(|error| format!("restore recovery intent removal failed: {error}"))?;
    sync_directory(&directory)
        .map_err(|error| format!("restore recovery completion sync failed: {error}"))?;
    remove_capsule(&capsule_path);
    Ok(())
}

fn digest_file(path: &Path) -> Result<String, String> {
    let mut file = open_no_follow(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("restore recovery capsule read failed: {error}"))?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn remove_capsule(path: &Path) {
    let _ = fs::remove_file(path);
    for suffix in ["-shm", "-wal", "-journal"] {
        let mut sidecar = path.as_os_str().to_owned();
        sidecar.push(suffix);
        let _ = fs::remove_file(PathBuf::from(sidecar));
    }
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> std::io::Result<()> {
    fs::File::open(path)?.sync_all()
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

fn recovery_directory(db_path: &str) -> Result<PathBuf, String> {
    let db = Path::new(db_path);
    let directory = db
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(directory)
        .map_err(|error| format!("restore recovery directory create failed: {error}"))?;
    Ok(directory.to_path_buf())
}

fn database_prefix(db_path: &str) -> Result<String, String> {
    Path::new(db_path)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| "restore database path has no filename".into())
}

fn intent_path(db_path: &str, directory: &Path) -> Result<PathBuf, String> {
    Ok(directory.join(format!(".{}{}", database_prefix(db_path)?, INTENT_SUFFIX)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn production_actor_rolls_back_an_interrupted_restore_before_ready() {
        let directory = tempfile::tempdir().unwrap();
        let db_path = directory.path().join("ark.db");
        let db_path_text = db_path.to_str().unwrap();
        let connection = crate::db::open_db(db_path_text).unwrap();
        crate::db::init_schema(&connection).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE restore_journal_test(value TEXT NOT NULL);
                 INSERT INTO restore_journal_test VALUES ('before');",
            )
            .unwrap();
        let expected_schema = super::super::schema_fingerprint(&connection).unwrap();
        let _journal = RestoreJournal::begin(&connection, db_path_text, &expected_schema).unwrap();
        connection
            .execute("UPDATE restore_journal_test SET value = 'partial'", [])
            .unwrap();
        drop(connection);

        let (handle, mut owner) =
            crate::db::DbHandle::open(db_path_text.to_owned(), "device-a".into())
                .await
                .unwrap();
        let value = handle.with_connection(|connection| {
            connection
                .query_row("SELECT value FROM restore_journal_test", [], |row| {
                    row.get::<_, String>(0)
                })
                .unwrap()
        });
        assert_eq!(value, "before");
        assert!(
            !intent_path(db_path_text, &recovery_directory(db_path_text).unwrap())
                .unwrap()
                .exists()
        );
        owner
            .shutdown(std::time::Instant::now() + std::time::Duration::from_secs(2))
            .await
            .unwrap();

        recover_interrupted_restore(db_path_text).unwrap();
    }

    #[test]
    fn invalid_capsule_keeps_intent_and_does_not_mutate_live_database() {
        let directory = tempfile::tempdir().unwrap();
        let db_path = directory.path().join("ark.db");
        let db_path_text = db_path.to_str().unwrap();
        let connection = Connection::open(&db_path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE restore_journal_test(value TEXT NOT NULL);
                 INSERT INTO restore_journal_test VALUES ('before');",
            )
            .unwrap();
        let expected_schema = super::super::schema_fingerprint(&connection).unwrap();
        let journal = RestoreJournal::begin(&connection, db_path_text, &expected_schema).unwrap();
        connection
            .execute("UPDATE restore_journal_test SET value = 'partial'", [])
            .unwrap();
        fs::write(journal.capsule_path(), b"corrupt capsule").unwrap();
        drop(journal);
        drop(connection);

        assert!(recover_interrupted_restore(db_path_text).is_err());
        let live = Connection::open(&db_path).unwrap();
        let value: String = live
            .query_row("SELECT value FROM restore_journal_test", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(value, "partial");
        assert!(
            intent_path(db_path_text, &recovery_directory(db_path_text).unwrap())
                .unwrap()
                .exists()
        );
    }
}
