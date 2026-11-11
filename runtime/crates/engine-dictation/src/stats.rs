// Persistence для агрегатной статистики диктации (как Raycast: WPM / Time
// Saved / Total Words). Отдельный JSON `<data_dir>/dictation-stats.json` —
// логически независимо от config'а (user prefs vs metrics). Schema additive:
// неизвестные поля игнорируются, отсутствующие принимают default.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::config::data_dir;

/// Допущение: средняя скорость machine-typing'а (для расчёта Time Saved).
/// Сознательно консервативное значение — кто-то печатает быстрее, кто-то
/// медленнее. 40 WPM ≈ медианный пользователь по open keystroke datasets.
pub const ASSUMED_TYPING_WPM: f64 = 40.0;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DictationStats {
    /// Сколько слов было успешно распознано суммарно за всю историю.
    pub total_words: u64,
    /// Сколько секунд аудио было записано (включая паузы). Это длительность
    /// записи на стороне renderer'а, не время API call'а.
    pub total_record_seconds: u64,
    /// Сколько успешных сессий распознавания было.
    pub total_sessions: u64,
    /// По дням, ключ — дата `YYYY-MM-DD` в UTC: из неё плагин рисует
    /// хит-мапу активности. Старые записи без этого поля читаются как пустые.
    pub days: BTreeMap<String, DayStats>,
}

/// Один день активности: слова, секунды записи, сессии.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DayStats {
    pub words: u64,
    pub seconds: u64,
    pub sessions: u64,
}

impl DictationStats {
    /// WPM = total_words / (total_record_minutes). 0 если record_seconds == 0.
    pub fn wpm(&self) -> f64 {
        if self.total_record_seconds == 0 {
            0.0
        } else {
            (self.total_words as f64) * 60.0 / (self.total_record_seconds as f64)
        }
    }

    /// Time saved (seconds): сколько ушло бы у пользователя на печать тех же
    /// слов со скоростью `ASSUMED_TYPING_WPM`, минус фактическая запись.
    /// Может быть отрицательным если запись была медленнее печати — обрезаем
    /// до 0 (стрелка часов вниз бессмысленна).
    pub fn time_saved_seconds(&self) -> u64 {
        let would_take = (self.total_words as f64) * 60.0 / ASSUMED_TYPING_WPM;
        let actual = self.total_record_seconds as f64;
        let saved = (would_take - actual).max(0.0);
        saved.round() as u64
    }

    /// Учесть результат успешной транскрипции. Считаем words как whitespace-
    /// разделённые токены (грубо но достаточно для агрегата).
    pub fn record_session(&mut self, transcript: &str, record_seconds: u64) {
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        self.record_session_on(transcript, record_seconds, &today);
    }

    /// То же, но с явной датой: тесты не зависят от часов машины.
    pub fn record_session_on(&mut self, transcript: &str, record_seconds: u64, day: &str) {
        let words = count_words(transcript);
        self.total_words = self.total_words.saturating_add(words);
        self.total_record_seconds = self.total_record_seconds.saturating_add(record_seconds);
        self.total_sessions = self.total_sessions.saturating_add(1);
        let bucket = self.days.entry(day.to_owned()).or_default();
        bucket.words = bucket.words.saturating_add(words);
        bucket.seconds = bucket.seconds.saturating_add(record_seconds);
        bucket.sessions = bucket.sessions.saturating_add(1);
        self.prune_days();
    }

    /// Хит-мапа показывает год; всё старше не храним, иначе файл растёт
    /// бесконечно. Ключи сортированы, поэтому старые — в начале.
    fn prune_days(&mut self) {
        const KEEP_DAYS: usize = 400;
        while self.days.len() > KEEP_DAYS {
            let oldest = self.days.keys().next().cloned();
            match oldest {
                Some(key) => {
                    self.days.remove(&key);
                }
                None => break,
            }
        }
    }
}

fn count_words(s: &str) -> u64 {
    s.split_whitespace().filter(|w| !w.is_empty()).count() as u64
}

pub fn stats_path() -> PathBuf {
    data_dir().join("dictation-stats.json")
}

pub fn load() -> DictationStats {
    load_from(&stats_path())
}

pub fn load_from(path: &Path) -> DictationStats {
    match std::fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => DictationStats::default(),
    }
}

pub fn save(stats: &DictationStats) -> std::io::Result<()> {
    save_to(&stats_path(), stats)
}

pub fn save_to(path: &Path, stats: &DictationStats) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(stats).map_err(std::io::Error::other)?;
    std::fs::write(path, text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wpm_zero_when_no_sessions() {
        let s = DictationStats::default();
        assert_eq!(s.wpm(), 0.0);
        assert_eq!(s.time_saved_seconds(), 0);
    }

    #[test]
    fn record_session_accumulates() {
        let mut s = DictationStats::default();
        s.record_session("привет мир", 5);
        assert_eq!(s.total_sessions, 1);
        assert_eq!(s.total_words, 2);
        assert_eq!(s.total_record_seconds, 5);

        s.record_session("hello world from dictation", 10);
        assert_eq!(s.total_sessions, 2);
        assert_eq!(s.total_words, 6);
        assert_eq!(s.total_record_seconds, 15);
    }

    #[test]
    fn wpm_calculates_from_words_per_minute() {
        // 200 слов за 60 секунд = 200 WPM.
        let s = DictationStats {
            total_words: 200,
            total_record_seconds: 60,
            total_sessions: 1,
            ..Default::default()
        };
        assert!((s.wpm() - 200.0).abs() < 0.01);
    }

    #[test]
    fn time_saved_positive_when_speech_faster_than_typing() {
        // 200 слов на 60s. Печатая 40 WPM, понадобилось бы 5 минут (300s).
        // Saved = 300 - 60 = 240s.
        let s = DictationStats {
            total_words: 200,
            total_record_seconds: 60,
            total_sessions: 1,
            ..Default::default()
        };
        assert_eq!(s.time_saved_seconds(), 240);
    }

    #[test]
    fn time_saved_zero_when_speech_slower_than_typing() {
        // 10 слов на 60s = 10 WPM. Печатая 40 WPM — 15s. Запись медленнее →
        // saved обрезается до 0 (не показываем отрицательное).
        let s = DictationStats {
            total_words: 10,
            total_record_seconds: 60,
            total_sessions: 1,
            ..Default::default()
        };
        assert_eq!(s.time_saved_seconds(), 0);
    }

    #[test]
    fn days_accumulate_per_date() {
        let mut s = DictationStats::default();
        s.record_session_on("один два", 4, "2026-11-10");
        s.record_session_on("три", 2, "2026-11-10");
        s.record_session_on("четыре пять шесть", 9, "2026-11-11");
        assert_eq!(
            s.days["2026-11-10"],
            DayStats {
                words: 3,
                seconds: 6,
                sessions: 2
            }
        );
        assert_eq!(
            s.days["2026-11-11"],
            DayStats {
                words: 3,
                seconds: 9,
                sessions: 1
            }
        );
        assert_eq!(s.total_sessions, 3);
    }

    #[test]
    fn old_days_are_pruned() {
        let mut s = DictationStats::default();
        for day in 1..=420 {
            s.record_session_on("слово", 1, &format!("2025-01-{day:03}"));
        }
        assert_eq!(s.days.len(), 400);
        assert!(!s.days.contains_key("2025-01-001"));
        assert_eq!(s.total_sessions, 420);
    }

    #[test]
    fn stats_without_days_load_as_empty() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let path = tmp.path().join("stats.json");
        std::fs::write(
            &path,
            r#"{"totalWords":12,"totalRecordSeconds":30,"totalSessions":2}"#,
        )
        .expect("write");
        let loaded = load_from(&path);
        assert_eq!(loaded.total_words, 12);
        assert!(loaded.days.is_empty());
    }

    #[test]
    fn save_load_roundtrip() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let path = tmp.path().join("stats.json");
        let mut s = DictationStats::default();
        s.record_session("один два три четыре", 12);
        save_to(&path, &s).expect("save");
        let loaded = load_from(&path);
        assert_eq!(loaded.total_words, 4);
        assert_eq!(loaded.total_record_seconds, 12);
        assert_eq!(loaded.total_sessions, 1);
    }

    #[test]
    fn load_missing_returns_default() {
        let tmp = tempfile::TempDir::new().expect("tempdir");
        let path = tmp.path().join("missing.json");
        let loaded = load_from(&path);
        assert_eq!(loaded.total_words, 0);
    }

    #[test]
    fn count_words_handles_punctuation_and_newlines() {
        assert_eq!(count_words(""), 0);
        assert_eq!(count_words("hello"), 1);
        assert_eq!(count_words("hello world"), 2);
        assert_eq!(count_words("hello\nworld\t!\n"), 3);
        assert_eq!(count_words("  множество   пробелов "), 2);
    }
}
