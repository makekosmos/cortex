use ark_store::writer::SqliteWriter;
use rusqlite::params;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const COMMITS_PER_TRIAL: usize = 700;
const TRIALS: usize = 5;
const PAYLOAD_BYTES: usize = 512;

struct TrialResult {
    elapsed: Duration,
    verified: bool,
    sqlite_version: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    let sqlite = run_trials()?;
    let median = median(sqlite.iter().map(|trial| trial.elapsed).collect());
    let min = sqlite
        .iter()
        .map(|trial| trial.elapsed)
        .min()
        .expect("at least one trial runs");
    let max = sqlite
        .iter()
        .map(|trial| trial.elapsed)
        .max()
        .expect("at least one trial runs");

    println!("os={}", std::env::consts::OS);
    println!(
        "sqlite_version={}",
        sqlite.first().expect("trials run").sqlite_version
    );
    println!("journal_mode=wal synchronous=FULL");
    println!("commits_per_trial={COMMITS_PER_TRIAL} payload_bytes={PAYLOAD_BYTES}");
    for (index, trial) in sqlite.iter().enumerate() {
        println!(
            "trial={} total_ms={:.3} avg_commit_us={:.3} verified={}",
            index + 1,
            trial.elapsed.as_secs_f64() * 1_000.0,
            trial.elapsed.as_secs_f64() * 1_000_000.0 / COMMITS_PER_TRIAL as f64,
            trial.verified,
        );
    }
    println!(
        "median_avg_commit_us={:.3} range_avg_commit_us={:.3}..{:.3}",
        median.as_secs_f64() * 1_000_000.0 / COMMITS_PER_TRIAL as f64,
        min.as_secs_f64() * 1_000_000.0 / COMMITS_PER_TRIAL as f64,
        max.as_secs_f64() * 1_000_000.0 / COMMITS_PER_TRIAL as f64,
    );
    Ok(())
}

fn run_trials() -> Result<Vec<TrialResult>, Box<dyn Error>> {
    (0..TRIALS).map(run_trial).collect::<Result<Vec<_>, _>>()
}

fn run_trial(index: usize) -> Result<TrialResult, Box<dyn Error>> {
    let path = database_path(index);
    let path_text = path.to_string_lossy();
    let mut writer = SqliteWriter::open(&path_text)?;
    let settings = writer.with_connection_mut(|connection| {
        let journal: String = connection.query_row("PRAGMA journal_mode", [], |row| row.get(0))?;
        let synchronous: i64 = connection.query_row("PRAGMA synchronous", [], |row| row.get(0))?;
        let sqlite_version: String =
            connection.query_row("SELECT sqlite_version()", [], |row| row.get(0))?;
        connection.execute_batch(
            "CREATE TABLE benchmark_write (
                id INTEGER PRIMARY KEY CHECK(id = 1), payload BLOB NOT NULL, writes INTEGER NOT NULL
             );
             INSERT INTO benchmark_write VALUES (1, zeroblob(512), 0);",
        )?;
        Ok::<_, rusqlite::Error>((journal, synchronous, sqlite_version))
    })?;
    if (settings.0.as_str(), settings.1) != ("wal", 2) {
        return Err(format!("unexpected production SQLite settings: {settings:?}").into());
    }

    // Warm schema/cache setup outside the measured interval.
    writer
        .immediate(|transaction| {
            transaction.execute(
                "UPDATE benchmark_write SET payload = ?1 WHERE id = 1",
                [vec![0_u8; PAYLOAD_BYTES]],
            )?;
            Ok::<_, rusqlite::Error>(())
        })
        .map_err(transaction_error)?;

    let started = Instant::now();
    for write in 0..COMMITS_PER_TRIAL {
        let payload = vec![(write % u8::MAX as usize) as u8; PAYLOAD_BYTES];
        let changed = writer
            .immediate(|transaction| {
                transaction.execute(
                    "UPDATE benchmark_write SET payload = ?1, writes = writes + 1 WHERE id = 1",
                    params![payload],
                )
            })
            .map_err(transaction_error)?;
        if changed != 1 {
            return Err(format!("commit {write} changed {changed} rows").into());
        }
    }
    let elapsed = started.elapsed();
    let verified = writer.with_connection_mut(|connection| {
        let (writes, payload): (i64, Vec<u8>) = connection.query_row(
            "SELECT writes, payload FROM benchmark_write WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let expected = vec![((COMMITS_PER_TRIAL - 1) % u8::MAX as usize) as u8; PAYLOAD_BYTES];
        Ok::<_, rusqlite::Error>(writes == COMMITS_PER_TRIAL as i64 && payload == expected)
    })?;
    drop(writer);
    remove_database_files(&path)?;
    if !verified {
        return Err(format!("trial {index} did not persist its expected final state").into());
    }
    Ok(TrialResult {
        elapsed,
        verified,
        sqlite_version: settings.2,
    })
}

fn database_path(index: usize) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is after Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "ark-sqlite-durability-{}-{nonce}-{index}.db",
        std::process::id()
    ))
}

fn remove_database_files(path: &Path) -> Result<(), std::io::Error> {
    for suffix in ["", "-shm", "-wal"] {
        let file = PathBuf::from(format!("{}{suffix}", path.to_string_lossy()));
        if file.exists() {
            fs::remove_file(file)?;
        }
    }
    Ok(())
}

fn transaction_error(
    error: ark_store::writer::TransactionFailure<rusqlite::Error>,
) -> std::io::Error {
    std::io::Error::other(format!("SQLite writer transaction failed: {error:?}"))
}

fn median(mut values: Vec<Duration>) -> Duration {
    values.sort_unstable();
    values[values.len() / 2]
}
