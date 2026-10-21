pub use ark_domain::app_identity::canonical_app_key;
use std::collections::HashSet;

// Canonical app identity + top-apps merge (KOS-287).
//
// `tracked_apps.id` historically hashed the full normalized exe path. Apps
// whose exe moves on update (Squirrel `app-1.0.x`, per-version browser dirs)
// got a new id every update — the same app appeared several times in
// analytics. The tracker now hashes the canonical identity below, and
// `load_usage_analytics` merges legacy per-path rows under the same key so
// already-accumulated data dedupes without touching stored records.

/// System-noise processes live under %SystemRoot% (explorer, sihost,
/// searchhost, textinputhost, dllhost, ...). Analytics marks rows instead of
/// dropping them — the UI hides them by default behind a toggle, and stored
/// records stay untouched.
pub fn is_system_path(normalized_exe_path: &str, windows_dir: &str) -> bool {
    let dir = windows_dir
        .trim()
        .trim_end_matches(['/', '\\'])
        .replace('/', "\\")
        .to_lowercase();
    if dir.is_empty() {
        return false;
    }
    normalized_exe_path
        .trim()
        .replace('/', "\\")
        .to_lowercase()
        .starts_with(&format!("{dir}\\"))
}

/// Candidate pickers (`list_recent_usage_processes`, `search_usage_processes`)
/// group by `tracked_apps.id`, which used to hash the full path — the same
/// game across version dirs produced duplicate picker rows. Collapse to one
/// candidate per canonical exe identity; callers order by last_seen_at DESC,
/// so the first row per key is the freshest.
fn dedup_usage_process_candidates(
    candidates: Vec<UsageProcessCandidate>,
) -> Vec<UsageProcessCandidate> {
    let mut seen = HashSet::new();
    candidates
        .into_iter()
        .filter(|candidate| {
            seen.insert(canonical_app_key(
                candidate.exe_path.as_deref().unwrap_or_default(),
                candidate.process_name.as_deref().unwrap_or_default(),
            ))
        })
        .collect()
}

fn newer_than(a: &Option<String>, b: &Option<String>) -> bool {
    // started_at/ended_at are RFC3339 — lexicographic compare is chronological.
    match (a, b) {
        (None, _) => false,
        (Some(_), None) => true,
        (Some(x), Some(y)) => x > y,
    }
}

/// Merge per-`tracked_apps.id` aggregates into one row per canonical exe
/// identity. Representative fields (name, path, icon, id) come from the row
/// with the freshest `last_seen_at` — the post-update path carries the
/// version-info display name; time and session counts are summed. Rows are
/// sorted by active time (foreground_ms: focused and not idle) descending.
pub fn merge_top_apps(
    rows: Vec<TopAppEntry>,
    limit: usize,
    windows_dir: Option<&str>,
) -> Vec<TopAppEntry> {
    let mut order: Vec<String> = Vec::new();
    let mut groups: HashMap<String, TopAppEntry> = HashMap::new();
    for row in rows {
        let key = canonical_app_key(&row.normalized_path, &row.process_name);
        match groups.get_mut(&key) {
            Some(acc) => {
                acc.runtime_ms = acc.runtime_ms.saturating_add(row.runtime_ms);
                acc.foreground_ms = acc.foreground_ms.saturating_add(row.foreground_ms);
                acc.idle_ms = acc.idle_ms.saturating_add(row.idle_ms);
                acc.sessions = acc.sessions.saturating_add(row.sessions);
                if newer_than(&row.last_seen_at, &acc.last_seen_at) {
                    acc.id = row.id;
                    acc.display_name = row.display_name;
                    acc.process_name = row.process_name;
                    acc.normalized_path = row.normalized_path;
                    acc.icon_ref = row.icon_ref.or_else(|| acc.icon_ref.take());
                    acc.last_seen_at = row.last_seen_at;
                } else if acc.icon_ref.is_none() {
                    acc.icon_ref = row.icon_ref;
                }
            }
            None => {
                order.push(key.clone());
                groups.insert(key, row);
            }
        }
    }
    let mut merged: Vec<TopAppEntry> = order
        .into_iter()
        .filter_map(|key| groups.remove(&key))
        .map(|mut row| {
            row.is_system =
                windows_dir.is_some_and(|dir| is_system_path(&row.normalized_path, dir));
            row
        })
        .collect();
    merged.sort_by(|a, b| {
        b.foreground_ms
            .cmp(&a.foreground_ms)
            .then_with(|| b.runtime_ms.cmp(&a.runtime_ms))
            .then_with(|| b.last_seen_at.cmp(&a.last_seen_at))
            .then_with(|| a.display_name.cmp(&b.display_name))
    });
    merged.truncate(limit);
    merged
}
