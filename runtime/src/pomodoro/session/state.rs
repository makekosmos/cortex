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
    pub title: String,
    pub tasks: Vec<TaskRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase_ends_at_ms: Option<u64>,
}

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
    phase: Phase,
    remaining_ms: u64,
    total_ms: u64,
    completed_pomodoros: u32,
    is_running: bool,
    is_paused: bool,
    phase_ends_at_ms: u64,
    last_config: Option<SessionConfig>,
}
