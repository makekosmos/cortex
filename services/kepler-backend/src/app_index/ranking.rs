// Ranking — name match + frecency boost.
//
// Simple v1: prefix > word-prefix > substring > none. Frecency multiplier
// из usage events (ARK usage_event_obj). BM25 / fuzzy — позже если нужно.

use crate::app_index::app::App;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize)]
pub struct ScoredApp {
    pub app: App,
    pub score: f64,
}

#[derive(Debug, Default, Clone)]
pub struct UsageStats {
    /// app_id → invocations за последние N дней (например 30).
    pub recent_invocations: HashMap<String, u32>,
}

impl UsageStats {
    pub fn empty() -> Self {
        Self::default()
    }
}

pub fn rank(apps: &[App], query: &str, limit: usize, usage: &UsageStats) -> Vec<ScoredApp> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }

    let mut scored: Vec<ScoredApp> = apps
        .iter()
        .filter_map(|a| {
            let name_score = name_match_score(&q, &a.name);
            if name_score <= 0.0 {
                return None;
            }
            let freq = usage.recent_invocations.get(&a.id).copied().unwrap_or(0);
            let frecency_boost = 1.0 + (1.0 + freq as f64).ln() * 0.4;
            let score = name_score * frecency_boost;
            Some(ScoredApp {
                app: a.clone(),
                score,
            })
        })
        .collect();

    scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(limit);
    scored
}

/// 1.0 = full prefix match, 0.7 = word prefix (after space/dash), 0.4 = substring,
/// 0.0 = no match. Case-insensitive (query уже lowercase, name lowercase здесь).
fn name_match_score(query: &str, name: &str) -> f64 {
    let name_lc = name.to_lowercase();
    if name_lc.starts_with(query) {
        return 1.0;
    }
    // Word prefix
    for sep in [' ', '-', '_', '.', '/', '\\'] {
        for part in name_lc.split(sep) {
            if part.starts_with(query) {
                return 0.7;
            }
        }
    }
    if name_lc.contains(query) {
        return 0.4;
    }
    0.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_index::app::AppKind;

    fn mk(id: &str, name: &str) -> App {
        App {
            id: id.into(),
            name: name.into(),
            exec_path: format!("/{name}"),
            icon_path: None,
            kind: AppKind::Win32,
            source: "test".into(),
            mtime: 0,
        }
    }

    #[test]
    fn prefix_wins_over_substring() {
        let apps = vec![mk("a", "Notepad"), mk("b", "MyNotepadFork")];
        let r = rank(&apps, "Note", 10, &UsageStats::empty());
        assert_eq!(r[0].app.id, "a");
        assert_eq!(r[1].app.id, "b");
    }

    #[test]
    fn frecency_boosts() {
        let apps = vec![mk("a", "Calculator"), mk("b", "Calc Plus")];
        let mut usage = UsageStats::empty();
        usage.recent_invocations.insert("b".into(), 10);
        let r = rank(&apps, "calc", 10, &usage);
        // "Calculator" — prefix (1.0), "Calc Plus" — word-prefix (0.7) * boost.
        // boost = 1 + ln(11)*0.4 ≈ 1 + 0.96 = ~1.96, итог b ≈ 1.37, a ≈ 1.0.
        assert_eq!(r[0].app.id, "b");
    }

    #[test]
    fn empty_query_returns_nothing() {
        let apps = vec![mk("a", "Notepad")];
        let r = rank(&apps, "", 10, &UsageStats::empty());
        assert!(r.is_empty());
    }
}
