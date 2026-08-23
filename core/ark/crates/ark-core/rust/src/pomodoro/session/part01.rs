// Pomodoro Session вЂ” state machine c parity Рє TS `usePomodoro`.
//
// ## РљРѕРЅС‚СЂР°РєС‚ TS-parity
//
// * `start(config)`: idle в†’ work (РёР»Рё paused в†’ resume current phase).
//   РЎРѕС…СЂР°РЅСЏРµС‚ config РІ `last_context` РґР»СЏ РїРѕСЃР»РµРґСѓСЋС‰РёС… transition'РѕРІ.
// * `pause()` / `resume()`: Р·Р°РјРѕСЂР°Р¶РёРІР°РµС‚ / СЂР°Р·РјРѕСЂРѕР·Рєa remaining_ms.
//   Р’ TS РёСЃРїРѕР»СЊР·СѓРµС‚СЃСЏ `phaseEndsAt = now + remainingMs` РїСЂРё resume вЂ”
//   РјС‹ РґРµР»Р°РµРј С‚Рѕ Р¶Рµ.
// * `tick()`: РІС‹Р·С‹РІР°РµС‚СЃСЏ ticker'РѕРј РєР°Р¶РґСѓСЋ СЃРµРєСѓРЅРґСѓ. Р•СЃР»Рё remaining_ms
//   РґРѕС€С‘Р» РґРѕ 0 вЂ” finish_phase(): increment completed (РµСЃР»Рё work
//   Р·Р°РІРµСЂС€РёР»СЃСЏ), РїРµСЂРµРєР»СЋС‡РµРЅРёРµ phase, РѕРїС†РёРѕРЅР°Р»СЊРЅРѕ auto-start.
// * `skip()`: РёРјРјРµРґРёР°С‚РЅРѕРµ Р·Р°РІРµСЂС€РµРЅРёРµ С‚РµРєСѓС‰РµР№ С„Р°Р·С‹ (РєР°Рє tick РІ РјРѕРјРµРЅС‚
//   remaining=0). Р•СЃР»Рё phase==idle вЂ” no-op.
// * `stop()`: Р¶С‘СЃС‚РєРёР№ reset вЂ” idle, completed=0.
//
// ## РЎРѕР±С‹С‚РёСЏ
//
// Р›СЋР±РѕР№ mutation С€Р»С‘С‚ events С‡РµСЂРµР· broadcast channel. Wrapper'С‹ РІ
// kepler-backend subscribe'Р°СЋС‚СЃСЏ Рё forward'СЏС‚ С‡РµСЂРµР· WS РєР»РёРµРЅС‚Р°Рј.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use super::clock::Clock;
use super::events::SessionEvent;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    Idle,
    Work,
    ShortBreak,
    LongBreak,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskRef {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    #[serde(rename = "workMin")]
    pub work_min: u32,
    #[serde(rename = "shortBreakMin")]
    pub short_break_min: u32,
    #[serde(rename = "longBreakMin")]
    pub long_break_min: u32,
    #[serde(rename = "pomodorosUntilLongBreak")]
    pub pomodoros_until_long_break: u32,
    #[serde(rename = "autoStartWork", default)]
    pub auto_start_work: bool,
    #[serde(rename = "autoStartBreak", default)]
    pub auto_start_break: bool,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub tasks: Vec<TaskRef>,
    /// Optional override РґР»СЏ work-С„Р°Р·С‹ вЂ” РёСЃРїРѕР»СЊР·СѓРµС‚СЃСЏ command bus action'Р°РјРё.
    #[serde(
        rename = "workMinOverride",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub work_min_override: Option<u32>,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            work_min: 25,
            short_break_min: 5,
            long_break_min: 15,
            pomodoros_until_long_break: 4,
            auto_start_work: false,
            auto_start_break: false,
            title: String::new(),
            tasks: Vec::new(),
            work_min_override: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionState {
    pub phase: Phase,
    pub remaining_ms: u64,
    pub total_ms: u64,
    pub completed_pomodoros: u32,
    pub is_running: bool,
    pub is_paused: bool,
    /// Title РїРѕСЃР»РµРґРЅРµРіРѕ start'Р° вЂ” renderer РёСЃРїРѕР»СЊР·СѓРµС‚ РґР»СЏ time_entry.
    pub title: String,
    /// Tasks вЂ” РґР»СЏ multi-task split renderer'РѕРј.
    pub tasks: Vec<TaskRef>,
    /// Wallclock (Unix ms) РєРѕРіРґР° С‚РµРєСѓС‰Р°СЏ С„Р°Р·Р° Р·Р°РєРѕРЅС‡РёС‚СЃСЏ. `Some` С‚РѕР»СЊРєРѕ
    /// РµСЃР»Рё `is_running && !is_paused` вЂ” renderer РёРЅС‚РµСЂРїРѕР»РёСЂСѓРµС‚
    /// `remainingMs = max(0, phase_ends_at_ms - Date.now())` Р»РѕРєР°Р»СЊРЅРѕ
    /// (smooth 30fps), backend С‚РёРєР°РµС‚ 1 Hz С‚РѕР»СЊРєРѕ РґР»СЏ consistency
    /// (is_running / completed_pomodoros / phase-boundary).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase_ends_at_ms: Option<u64>,
}

/// Persisted snapshot вЂ” С‡С‚Рѕ РјС‹ РїРёС€РµРј РІ `<data_dir>/pomodoro-state.json`.
/// РћС‚РґРµР»СЊРЅР°СЏ struct РѕС‚ `SessionState`, С‚Р°Рє РєР°Рє РЅСѓР¶РЅРѕ СЃРѕС…СЂР°РЅСЏС‚СЊ `last_config`
/// (РґР»СЏ advance Р»РѕРіРёРєРё РЅР° restore) + `phase_ends_at_ms` РІСЃРµРіРґР° (Р° РЅРµ С‚РѕР»СЊРєРѕ
/// РєРѕРіРґР° running) вЂ” РёРЅР°С‡Рµ РЅР° restore РёР· РїР°СѓР·С‹ РЅРµС‡РµРіРѕ РїРѕРєР°Р·Р°С‚СЊ.
///
/// `version` вЂ” РґР»СЏ Р±СѓРґСѓС‰РёС… РјРёРіСЂР°С†РёР№. РЎРµР№С‡Р°СЃ РІСЃРµРіРґР° `1`. РќР° load РµСЃР»Рё version
/// РЅРµ СЃРѕРІРїР°РґР°РµС‚ вЂ” host Р»РѕРіРёСЂСѓРµС‚ warning Рё РЅРµ СЂРµСЃС‚Р°РІСЂРёСЂСѓРµС‚.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistedSession {
    pub version: u32,
    pub phase: Phase,
    pub remaining_ms: u64,
    pub total_ms: u64,
    pub completed_pomodoros: u32,
    pub is_running: bool,
    pub is_paused: bool,
    /// Wallclock (Unix ms) РєРѕРіРґР° running phase РґРѕР»Р¶РЅР° Р·Р°РєРѕРЅС‡РёС‚СЊСЃСЏ. РЎРѕС…СЂР°РЅСЏРµРј
    /// РІСЃРµРіРґР° вЂ” РЅР° restore advance Р»РѕРіРёРєР° СЃРјРѕС‚СЂРёС‚ РЅР° РЅРµРіРѕ РѕС‚РЅРѕСЃРёС‚РµР»СЊРЅРѕ now.
    pub phase_ends_at_ms: u64,
    pub last_config: Option<SessionConfig>,
}

impl PersistedSession {
    pub const CURRENT_VERSION: u32 = 1;
}

impl SessionState {
    pub fn idle() -> Self {
        Self {
            phase: Phase::Idle,
            remaining_ms: 0,
            total_ms: 0,
            completed_pomodoros: 0,
            is_running: false,
            is_paused: false,
            title: String::new(),
            tasks: Vec::new(),
            phase_ends_at_ms: None,
        }
    }
}

pub struct Session {
    clock: Arc<dyn Clock>,
    events_tx: broadcast::Sender<SessionEvent>,
    // Mutable state.
    phase: Phase,
    remaining_ms: u64,
    total_ms: u64,
    completed_pomodoros: u32,
    is_running: bool,
    is_paused: bool,
    /// РџСЂРё `is_running && !is_paused` вЂ” wall-clock РјРѕРјРµРЅС‚ РІ ms РєРѕРіРґР°
    /// С‚РµРєСѓС‰Р°СЏ С„Р°Р·Р° Р·Р°РєРѕРЅС‡РёС‚СЃСЏ. TS-impl: `phaseEndsAt`.
    phase_ends_at_ms: u64,
    last_config: Option<SessionConfig>,
}


