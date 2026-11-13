//! A line a person can read afterwards, when a key or a click did nothing.

use std::io::Write as _;

/// Appends `line` to `<data home>/logs/<name>.log`, stamped with the second it
/// happened. A missing data home or a read-only disk is not an error: the log is
/// a courtesy, and the caller has nothing to do about it.
pub fn append(name: &str, line: &str) {
    let Some(home) = crate::engine::data_home() else {
        return;
    };
    let dir = home.join("logs");
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(format!("{name}.log")))
    {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|span| span.as_secs())
            .unwrap_or(0);
        let _ = writeln!(file, "{stamp} {line}");
    }
}
