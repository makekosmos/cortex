//! Daedalus agent runtime.
//!
//! Owns Codex app-server children and local Git worktrees. Nothing in this
//! module writes to ARK: durable state lives in the extension-scoped SQLite DB.

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStdout, Command};
use tokio::sync::{broadcast, mpsc};
use uuid::Uuid;

const MAX_OUTPUT_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: String,
    pub dirty: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Starting,
    Running,
    WaitingApproval,
    Completed,
    Interrupted,
    Failed,
    Archived,
}

impl SessionStatus {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Running => "running",
            Self::WaitingApproval => "waiting_approval",
            Self::Completed => "completed",
            Self::Interrupted => "interrupted",
            Self::Failed => "failed",
            Self::Archived => "archived",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub prompt: String,
    pub mode: String,
    pub model: Option<String>,
    pub status: String,
    pub branch: String,
    pub worktree_path: String,
    pub worktree_exists: bool,
    pub base_commit: String,
    pub codex_thread_id: Option<String>,
    pub active_turn_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub archived_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineEvent {
    pub id: i64,
    pub session_id: String,
    pub kind: String,
    pub payload: Value,
    pub created_at: String,
    pub updated_at: String,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Approval {
    pub id: String,
    pub session_id: String,
    pub request_id: Value,
    pub method: String,
    pub params: Value,
    pub status: String,
    pub response: Option<Value>,
    pub created_at: String,
    pub resolved_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentsEvent {
    pub event: &'static str,
    pub seq: u64,
    pub kind: String,
    pub session_id: String,
    pub payload: Value,
}

#[derive(Debug, Clone)]
struct RuntimeHandle {
    tx: mpsc::Sender<AppCommand>,
}

#[derive(Debug, Clone)]
struct StreamBuffer {
    event: TimelineEvent,
    last_emit: Instant,
    pending: bool,
}

struct AppServerStartup {
    child: Child,
    stdin: tokio::process::ChildStdin,
    lines: Lines<BufReader<ChildStdout>>,
    thread_id: String,
    active_turn_id: Option<String>,
    status: SessionStatus,
    buffered: Vec<Value>,
}

#[derive(Debug)]
enum AppCommand {
    Send {
        text: String,
    },
    Interrupt,
    Approval {
        request_id: Value,
        result: Value,
        done: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
    Shutdown(Option<tokio::sync::oneshot::Sender<()>>),
}

pub struct AgentsService {
    root: PathBuf,
    db: Mutex<Connection>,
    runtimes: Mutex<HashMap<String, RuntimeHandle>>,
    events: broadcast::Sender<Value>,
    seq: AtomicU64,
    event_order: Mutex<()>,
    recent_events: Mutex<VecDeque<Value>>,
    restored: AtomicBool,
    streams: Mutex<HashMap<(String, String), StreamBuffer>>,
}

impl AgentsService {
    pub fn new(data_dir: &Path) -> Result<Arc<Self>, String> {
        let (events, _) = broadcast::channel(512);
        Self::new_with_events(data_dir, events)
    }

    pub fn new_with_events(
        data_dir: &Path,
        events: broadcast::Sender<Value>,
    ) -> Result<Arc<Self>, String> {
        let root = data_dir.join("extensions-data").join("daedalus");
        std::fs::create_dir_all(root.join("worktrees")).map_err(|e| e.to_string())?;
        let conn = Connection::open(root.join("daedalus.db")).map_err(|e| e.to_string())?;
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(|e| e.to_string())?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS projects (
                id TEXT PRIMARY KEY, name TEXT NOT NULL, path TEXT NOT NULL UNIQUE,
                created_at TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY, project_id TEXT NOT NULL, title TEXT NOT NULL,
                prompt TEXT NOT NULL, mode TEXT NOT NULL, model TEXT, status TEXT NOT NULL,
                branch TEXT NOT NULL, worktree_path TEXT NOT NULL, base_commit TEXT NOT NULL,
                codex_thread_id TEXT, active_turn_id TEXT, created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL, archived_at TEXT
             );
             CREATE TABLE IF NOT EXISTS timeline (
                id INTEGER PRIMARY KEY AUTOINCREMENT, session_id TEXT NOT NULL,
                kind TEXT NOT NULL, payload_json TEXT NOT NULL, truncated INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL, updated_at TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_timeline_session_id ON timeline(session_id, id);
             CREATE TABLE IF NOT EXISTS approvals (
                id TEXT PRIMARY KEY, session_id TEXT NOT NULL, request_id_json TEXT NOT NULL,
                method TEXT NOT NULL, params_json TEXT NOT NULL, status TEXT NOT NULL,
                response_json TEXT, created_at TEXT NOT NULL, resolved_at TEXT
             );
             CREATE TABLE IF NOT EXISTS runtime_meta (
                key TEXT PRIMARY KEY, value INTEGER NOT NULL
             );
             INSERT OR IGNORE INTO runtime_meta(key,value) VALUES('event_seq',0);",
        )
        .map_err(|e| e.to_string())?;
        let initial_seq = conn
            .query_row(
                "SELECT value FROM runtime_meta WHERE key='event_seq'",
                [],
                |row| row.get::<_, u64>(0),
            )
            .unwrap_or(0);
        Ok(Arc::new(Self {
            root,
            db: Mutex::new(conn),
            runtimes: Mutex::new(HashMap::new()),
            events,
            seq: AtomicU64::new(initial_seq),
            event_order: Mutex::new(()),
            recent_events: Mutex::new(VecDeque::with_capacity(512)),
            restored: AtomicBool::new(false),
            streams: Mutex::new(HashMap::new()),
        }))
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Value> {
        self.events.subscribe()
    }

    pub async fn shutdown(&self) {
        let handles = {
            self.runtimes()
                .drain()
                .map(|(_, handle)| handle)
                .collect::<Vec<_>>()
        };
        let mut completions = Vec::with_capacity(handles.len());
        for handle in handles {
            let (done_tx, done_rx) = tokio::sync::oneshot::channel();
            if handle
                .tx
                .send(AppCommand::Shutdown(Some(done_tx)))
                .await
                .is_ok()
            {
                completions.push(done_rx);
            }
        }
        for completion in completions {
            let _ = tokio::time::timeout(Duration::from_secs(5), completion).await;
        }
    }

    fn db(&self) -> MutexGuard<'_, Connection> {
        self.db
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn runtimes(&self) -> MutexGuard<'_, HashMap<String, RuntimeHandle>> {
        self.runtimes
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn emit(&self, kind: &str, session_id: &str, payload: Value) {
        let _order = self.event_order.lock().unwrap_or_else(|p| p.into_inner());
        let seq = self
            .db()
            .query_row(
                "UPDATE runtime_meta SET value=value+1 WHERE key='event_seq' RETURNING value",
                [],
                |row| row.get::<_, u64>(0),
            )
            .unwrap_or_else(|_| self.seq.fetch_add(1, Ordering::Relaxed) + 1);
        self.seq.store(seq, Ordering::Release);
        let event = AgentsEvent {
            event: "agents_event",
            seq,
            kind: kind.to_string(),
            session_id: session_id.to_string(),
            payload,
        };
        if let Ok(value) = serde_json::to_value(event) {
            let mut recent = self.recent_events.lock().unwrap_or_else(|p| p.into_inner());
            if recent.len() == 512 {
                recent.pop_front();
            }
            recent.push_back(value.clone());
            let _ = self.events.send(value);
        }
    }

    pub async fn handle(self: &Arc<Self>, op: &str, params: Value) -> Result<Value, String> {
        self.restore_active_sessions().await;
        match op {
            "projects.list" => Ok(json!(self.list_projects()?)),
            "projects.add" => self.add_project(&required_str(&params, "path")?).await,
            "projects.remove" => self.remove_project(&required_str(&params, "project_id")?),
            "sessions.list" => Ok(json!(self.list_sessions(
                params
                    .get("include_archived")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            )?)),
            "sessions.get" => Ok(json!(
                self.get_session(&required_str(&params, "session_id")?)?
            )),
            "sessions.create" => self.create_session(params).await,
            "sessions.send" => {
                self.send(
                    &required_str(&params, "session_id")?,
                    &required_str(&params, "text")?,
                )
                .await
            }
            "sessions.interrupt" => self.interrupt(&required_str(&params, "session_id")?).await,
            "sessions.archive" => self.archive(&required_str(&params, "session_id")?),
            "sessions.remove_worktree" => {
                self.remove_worktree(&required_str(&params, "session_id")?)
                    .await
            }
            "sessions.timeline" => self.timeline(params),
            "approvals.respond" => self.respond_approval(params).await,
            "diff.get" => self.diff(&required_str(&params, "session_id")?).await,
            "models.list" => self.models().await,
            "editors.list" => self.editors().await,
            "editors.open" => self.open_editor(params).await,
            "snapshot" => self.snapshot(params),
            _ => Err(format!("agents.{op}: unknown sub-operation")),
        }
    }

    async fn restore_active_sessions(self: &Arc<Self>) {
        if self.restored.swap(true, Ordering::AcqRel) {
            return;
        }
        let sessions = match self.list_sessions(false) {
            Ok(sessions) => sessions,
            Err(error) => {
                tracing::warn!(%error, "failed to load Daedalus sessions for restore");
                self.restored.store(false, Ordering::Release);
                return;
            }
        };
        for session in sessions.into_iter().filter(|session| {
            matches!(
                session.status.as_str(),
                "starting" | "running" | "waiting_approval"
            )
        }) {
            if let Err(error) = self.spawn_runtime(session.clone()).await {
                tracing::warn!(session_id = %session.id, %error, "failed to restore Daedalus session");
                let _ = self.set_status(&session.id, SessionStatus::Failed);
            }
        }
    }

    fn list_projects(&self) -> Result<Vec<Project>, String> {
        let db = self.db();
        let mut statement = db
            .prepare("SELECT id,name,path,created_at FROM projects ORDER BY created_at")
            .map_err(|e| e.to_string())?;
        let rows = statement
            .query_map([], |row| {
                let path: String = row.get(2)?;
                Ok(Project {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    dirty: git_dirty(Path::new(&path)),
                    path,
                    created_at: row.get(3)?,
                })
            })
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }

    async fn add_project(&self, raw_path: &str) -> Result<Value, String> {
        let requested = PathBuf::from(raw_path);
        let root = git_output(&requested, &["rev-parse", "--show-toplevel"]).await?;
        let canonical =
            std::fs::canonicalize(root.trim()).map_err(|e| format!("repository path: {e}"))?;
        let path = canonical.to_string_lossy().into_owned();
        let name = canonical
            .file_name()
            .and_then(|v| v.to_str())
            .unwrap_or("Repository")
            .to_string();
        let id = hex_hash(&path)[..16].to_string();
        let created_at = now();
        self.db()
            .execute(
                "INSERT INTO projects(id,name,path,created_at) VALUES(?1,?2,?3,?4)
             ON CONFLICT(path) DO UPDATE SET name=excluded.name",
                params![id, name, path, created_at],
            )
            .map_err(|e| e.to_string())?;
        let project = self
            .list_projects()?
            .into_iter()
            .find(|p| p.id == id)
            .ok_or("project was not persisted")?;
        Ok(json!(project))
    }

    fn remove_project(&self, id: &str) -> Result<Value, String> {
        let active: i64 = self
            .db()
            .query_row(
                "SELECT COUNT(*) FROM sessions WHERE project_id=?1 AND archived_at IS NULL",
                [id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        if active > 0 {
            return Err("Нельзя удалить проект с активными сессиями".into());
        }
        self.db()
            .execute("DELETE FROM projects WHERE id=?1", [id])
            .map_err(|e| e.to_string())?;
        Ok(json!(true))
    }

    fn list_sessions(&self, include_archived: bool) -> Result<Vec<Session>, String> {
        let sql = if include_archived {
            "SELECT id,project_id,title,prompt,mode,model,status,branch,worktree_path,base_commit,codex_thread_id,active_turn_id,created_at,updated_at,archived_at FROM sessions ORDER BY updated_at DESC"
        } else {
            "SELECT id,project_id,title,prompt,mode,model,status,branch,worktree_path,base_commit,codex_thread_id,active_turn_id,created_at,updated_at,archived_at FROM sessions WHERE archived_at IS NULL ORDER BY updated_at DESC"
        };
        let db = self.db();
        let mut statement = db.prepare(sql).map_err(|e| e.to_string())?;
        let rows = statement
            .query_map([], session_from_row)
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }

    fn get_session(&self, id: &str) -> Result<Session, String> {
        self.db().query_row(
            "SELECT id,project_id,title,prompt,mode,model,status,branch,worktree_path,base_commit,codex_thread_id,active_turn_id,created_at,updated_at,archived_at FROM sessions WHERE id=?1",
            [id], session_from_row,
        ).optional().map_err(|e| e.to_string())?.ok_or_else(|| "Сессия не найдена".into())
    }

    async fn create_session(self: &Arc<Self>, input: Value) -> Result<Value, String> {
        let project_id = required_str(&input, "project_id")?;
        let prompt = required_str(&input, "prompt")?;
        let mode = input
            .get("mode")
            .and_then(Value::as_str)
            .unwrap_or("default")
            .to_string();
        if !matches!(mode.as_str(), "default" | "auto-review" | "full-access") {
            return Err("Неизвестный режим".into());
        }
        if mode == "full-access"
            && input.get("full_access_confirmed").and_then(Value::as_bool) != Some(true)
        {
            return Err("Для full-access требуется явное подтверждение".into());
        }
        let project = self
            .list_projects()?
            .into_iter()
            .find(|p| p.id == project_id)
            .ok_or("Проект не найден")?;
        let repo = PathBuf::from(&project.path);
        let base_commit = git_output(&repo, &["rev-parse", "HEAD"])
            .await?
            .trim()
            .to_string();
        let id = Uuid::new_v4().to_string();
        let short_id = id.chars().take(8).collect::<String>();
        let branch = format!("codex/{}-{short_id}", prompt_slug(&prompt));
        let worktree = self
            .root
            .join("worktrees")
            .join(hex_hash(&project.path))
            .join(&id);
        if let Some(parent) = worktree.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        git_status(
            &repo,
            &[
                "worktree",
                "add",
                "-b",
                &branch,
                path_str(&worktree)?,
                &base_commit,
            ],
        )
        .await?;
        let timestamp = now();
        let title = prompt
            .lines()
            .next()
            .unwrap_or("Новая задача")
            .chars()
            .take(80)
            .collect::<String>();
        let model = input
            .get("model")
            .and_then(Value::as_str)
            .map(str::to_string);
        self.db().execute(
            "INSERT INTO sessions(id,project_id,title,prompt,mode,model,status,branch,worktree_path,base_commit,created_at,updated_at)
             VALUES(?1,?2,?3,?4,?5,?6,'starting',?7,?8,?9,?10,?10)",
            params![id, project_id, title, prompt, mode, model, branch, worktree.to_string_lossy(), base_commit, timestamp],
        ).map_err(|e| e.to_string())?;
        self.append_timeline(&id, "user_message", json!({"text": prompt}), false)?;
        let session = self.get_session(&id)?;
        self.emit("session_updated", &id, json!(session));
        if let Err(error) = self.spawn_runtime(session.clone()).await {
            self.set_status(&id, SessionStatus::Failed)?;
            self.append_timeline(&id, "error", json!({"message": error}), false)?;
            self.emit("session_updated", &id, json!(self.get_session(&id)?));
            return Err(error);
        }
        Ok(json!(self.get_session(&id)?))
    }

    async fn spawn_runtime(self: &Arc<Self>, session: Session) -> Result<(), String> {
        let (tx, rx) = mpsc::channel(64);
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let service = self.clone();
        let id = session.id.clone();
        tokio::spawn(async move {
            run_app_server(service, session, rx, ready_tx).await;
        });
        ready_rx
            .await
            .map_err(|_| "Codex app-server завершился при запуске".to_string())??;
        self.runtimes().insert(id, RuntimeHandle { tx });
        Ok(())
    }

    async fn send(self: &Arc<Self>, session_id: &str, text: &str) -> Result<Value, String> {
        let event =
            self.append_timeline(session_id, "user_message", json!({"text": text}), false)?;
        self.emit("timeline_appended", session_id, json!(event));
        let mut tx = self.runtimes().get(session_id).map(|h| h.tx.clone());
        if tx.is_none() {
            let session = self.get_session(session_id)?;
            if session.archived_at.is_some() {
                return Err("Архивная сессия недоступна для продолжения".into());
            }
            self.spawn_runtime(session).await?;
            tx = self.runtimes().get(session_id).map(|h| h.tx.clone());
        }
        let tx = tx.ok_or("Сессия Codex не запущена")?;
        tx.send(AppCommand::Send {
            text: text.to_string(),
        })
        .await
        .map_err(|_| "Codex app-server недоступен".to_string())?;
        Ok(json!(true))
    }

    async fn interrupt(&self, session_id: &str) -> Result<Value, String> {
        if self.get_session(session_id)?.active_turn_id.is_none() {
            return Err("У сессии нет активного хода".into());
        }
        let tx = self
            .runtimes()
            .get(session_id)
            .map(|h| h.tx.clone())
            .ok_or("Сессия Codex не запущена")?;
        tx.send(AppCommand::Interrupt)
            .await
            .map_err(|_| "Codex app-server недоступен".to_string())?;
        self.set_status(session_id, SessionStatus::Interrupted)?;
        self.emit(
            "session_updated",
            session_id,
            json!(self.get_session(session_id)?),
        );
        Ok(json!(true))
    }

    fn archive(&self, session_id: &str) -> Result<Value, String> {
        let timestamp = now();
        self.db()
            .execute(
                "UPDATE sessions SET status='archived',archived_at=?2,updated_at=?2 WHERE id=?1",
                params![session_id, timestamp],
            )
            .map_err(|e| e.to_string())?;
        self.emit(
            "session_updated",
            session_id,
            json!(self.get_session(session_id)?),
        );
        Ok(json!(true))
    }

    async fn remove_worktree(&self, session_id: &str) -> Result<Value, String> {
        let session = self.get_session(session_id)?;
        if session.archived_at.is_none() {
            return Err("Сначала архивируйте сессию".into());
        }
        if session.active_turn_id.is_some() {
            return Err("Нельзя удалить worktree активной сессии".into());
        }
        if !session.worktree_exists {
            return Ok(json!(true));
        }
        if git_dirty(Path::new(&session.worktree_path)) {
            return Err("Worktree содержит незакоммиченные изменения".into());
        }
        let runtime = { self.runtimes().remove(session_id) };
        if let Some(handle) = runtime {
            let (done_tx, done_rx) = tokio::sync::oneshot::channel();
            handle
                .tx
                .send(AppCommand::Shutdown(Some(done_tx)))
                .await
                .map_err(|_| "Codex app-server недоступен".to_string())?;
            let _ = tokio::time::timeout(Duration::from_secs(5), done_rx).await;
        }
        let project = self
            .list_projects()?
            .into_iter()
            .find(|project| project.id == session.project_id)
            .ok_or("Проект не найден")?;
        git_status(
            Path::new(&project.path),
            &["worktree", "remove", &session.worktree_path],
        )
        .await?;
        self.emit(
            "session_updated",
            session_id,
            json!(self.get_session(session_id)?),
        );
        Ok(json!(true))
    }

    fn timeline(&self, input: Value) -> Result<Value, String> {
        let session_id = required_str(&input, "session_id")?;
        let cursor = input
            .get("cursor")
            .and_then(Value::as_i64)
            .unwrap_or(i64::MAX);
        let limit = input
            .get("limit")
            .and_then(Value::as_u64)
            .unwrap_or(100)
            .clamp(1, 500) as i64;
        let db = self.db();
        let mut statement = db.prepare(
            "SELECT id,session_id,kind,payload_json,created_at,updated_at,truncated FROM timeline WHERE session_id=?1 AND id<?2 ORDER BY id DESC LIMIT ?3"
        ).map_err(|e| e.to_string())?;
        let rows = statement
            .query_map(params![session_id, cursor, limit], timeline_from_row)
            .map_err(|e| e.to_string())?;
        let mut events = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        events.reverse();
        let next_cursor = events.first().map(|event| event.id);
        Ok(json!({"events":events,"nextCursor":next_cursor}))
    }

    async fn respond_approval(&self, input: Value) -> Result<Value, String> {
        let approval_id = required_str(&input, "approval_id")?;
        let decision = input
            .get("decision")
            .cloned()
            .unwrap_or_else(|| json!("decline"));
        let approval = self.get_approval(&approval_id)?;
        if approval.status != "pending" {
            return Err("Approval уже обработан".into());
        }
        let result = if approval.method == "item/tool/requestUserInput" {
            json!({"answers": input.get("answers").cloned().unwrap_or_else(|| json!({}))})
        } else if approval.method == "item/permissions/requestApproval" {
            if decision == json!("accept") {
                json!({"permissions": approval.params.get("permissions").cloned().unwrap_or_else(|| json!({})), "scope":"turn"})
            } else {
                json!({"permissions": {}})
            }
        } else {
            json!({"decision": decision})
        };
        let tx = self
            .runtimes()
            .get(&approval.session_id)
            .map(|h| h.tx.clone())
            .ok_or("Сессия Codex не запущена")?;
        let (done_tx, done_rx) = tokio::sync::oneshot::channel();
        tx.send(AppCommand::Approval {
            request_id: approval.request_id.clone(),
            result: result.clone(),
            done: done_tx,
        })
        .await
        .map_err(|_| "Codex app-server недоступен".to_string())?;
        tokio::time::timeout(Duration::from_secs(5), done_rx)
            .await
            .map_err(|_| "Codex app-server не подтвердил ответ".to_string())?
            .map_err(|_| "Codex app-server завершился".to_string())??;
        let timestamp = now();
        self.db().execute("UPDATE approvals SET status='resolved',response_json=?2,resolved_at=?3 WHERE id=?1", params![approval_id,result.to_string(),timestamp]).map_err(|e| e.to_string())?;
        let remaining: i64 = self
            .db()
            .query_row(
                "SELECT COUNT(*) FROM approvals WHERE session_id=?1 AND status='pending'",
                [&approval.session_id],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        self.set_status(
            &approval.session_id,
            if remaining == 0 {
                SessionStatus::Running
            } else {
                SessionStatus::WaitingApproval
            },
        )?;
        self.emit(
            "approval_resolved",
            &approval.session_id,
            json!({"approvalId":approval_id,"response":result}),
        );
        Ok(json!(true))
    }

    async fn diff(&self, session_id: &str) -> Result<Value, String> {
        let session = self.get_session(session_id)?;
        let cwd = PathBuf::from(&session.worktree_path);
        let canonical_cwd = std::fs::canonicalize(&cwd).map_err(|error| error.to_string())?;
        let status = git_output(&cwd, &["status", "--porcelain=v1", "-z"]).await?;
        let base_status =
            git_output(&cwd, &["diff", "--name-status", "-z", &session.base_commit]).await?;
        let mut unified = String::new();
        let mut files = Vec::new();
        let mut changed = BTreeMap::<String, String>::new();
        for record in status.split('\0').filter(|entry| entry.len() >= 4) {
            changed.insert(record[3..].to_string(), record[..2].to_string());
        }
        let mut fields = base_status.split('\0').filter(|entry| !entry.is_empty());
        while let (Some(state), Some(first_path)) = (fields.next(), fields.next()) {
            let path = if state.starts_with('R') || state.starts_with('C') {
                fields.next().unwrap_or(first_path)
            } else {
                first_path
            };
            changed
                .entry(path.to_string())
                .or_insert_with(|| state.to_string());
        }
        for (path, state) in changed {
            if Path::new(&path).components().any(|component| {
                matches!(
                    component,
                    std::path::Component::ParentDir
                        | std::path::Component::RootDir
                        | std::path::Component::Prefix(_)
                )
            }) {
                tracing::warn!(%path, session_id, "ignored unsafe Git path in Daedalus diff");
                continue;
            }
            let full = cwd.join(&path);
            let metadata = std::fs::symlink_metadata(&full).ok();
            let symlink = metadata
                .as_ref()
                .map(|metadata| metadata.file_type().is_symlink())
                .unwrap_or(false);
            let escapes_worktree = full.exists()
                && std::fs::canonicalize(&full)
                    .map(|canonical| !canonical.starts_with(&canonical_cwd))
                    .unwrap_or(true);
            let mut size = metadata.as_ref().map(std::fs::Metadata::len).unwrap_or(0);
            let binary = symlink || escapes_worktree || is_binary(&full);
            if state == "??" && !binary && size <= MAX_OUTPUT_BYTES as u64 {
                if let Ok(content) = std::fs::read_to_string(&full) {
                    unified.push_str(&untracked_patch(&path, &content));
                }
            } else if state != "??" && !binary {
                let base_object = format!("{}:{path}", session.base_commit);
                let base_size = git_output(&cwd, &["cat-file", "-s", &base_object])
                    .await
                    .ok()
                    .and_then(|value| value.trim().parse::<u64>().ok())
                    .unwrap_or(0);
                size = size.max(base_size);
                if size <= MAX_OUTPUT_BYTES as u64 {
                    unified.push_str(
                        &git_output(
                            &cwd,
                            &["diff", "--no-ext-diff", &session.base_commit, "--", &path],
                        )
                        .await?,
                    );
                }
            }
            files.push(json!({"path":path,"status":state,"size":size,"binary":binary,"contentOmitted":binary || size > MAX_OUTPUT_BYTES as u64}));
        }
        let truncated = unified.len() > MAX_OUTPUT_BYTES;
        let diff = truncate_utf8(unified, MAX_OUTPUT_BYTES);
        Ok(
            json!({"sessionId":session_id,"baseCommit":session.base_commit,"files":files,"unifiedDiff":diff,"truncated":truncated}),
        )
    }

    async fn models(&self) -> Result<Value, String> {
        let output = codex_one_shot("model/list", json!({"limit":100})).await?;
        Ok(output)
    }

    async fn editors(&self) -> Result<Value, String> {
        let mut editors = Vec::new();
        for (id, label, command) in [
            ("code", "Visual Studio Code", "code"),
            ("cursor", "Cursor", "cursor"),
            ("windsurf", "Windsurf", "windsurf"),
        ] {
            if command_available(command).await {
                editors.push(json!({"id":id,"label":label}));
            }
        }
        editors.push(json!({"id":"explorer","label":"Проводник"}));
        Ok(json!(editors))
    }

    async fn open_editor(&self, input: Value) -> Result<Value, String> {
        let session = self.get_session(&required_str(&input, "session_id")?)?;
        let editor = input
            .get("editor_id")
            .and_then(Value::as_str)
            .unwrap_or("explorer");
        let executable = match editor {
            "explorer" => "explorer.exe",
            "code" => "code",
            "cursor" => "cursor",
            "windsurf" => "windsurf",
            _ => return Err("Неизвестный редактор".into()),
        };
        if editor != "explorer" && !command_available(executable).await {
            return Err("Редактор не установлен".into());
        }
        let mut command = Command::new(executable);
        command
            .arg(&session.worktree_path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Не удалось открыть редактор: {e}"))?;
        Ok(json!(true))
    }

    fn snapshot(&self, input: Value) -> Result<Value, String> {
        let after_seq = input.get("after_seq").and_then(Value::as_u64).unwrap_or(0);
        let approvals = self.pending_approvals()?;
        let recent_events = self
            .recent_events
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .iter()
            .filter(|event| event.get("seq").and_then(Value::as_u64).unwrap_or(0) > after_seq)
            .cloned()
            .collect::<Vec<_>>();
        Ok(
            json!({"seq":self.seq.load(Ordering::Relaxed),"afterSeq":after_seq,"recentEvents":recent_events,"projects":self.list_projects()?,"sessions":self.list_sessions(false)?,"approvals":approvals}),
        )
    }

    fn append_timeline(
        &self,
        session_id: &str,
        kind: &str,
        mut payload: Value,
        mut truncated: bool,
    ) -> Result<TimelineEvent, String> {
        if let Some(text) = payload
            .get_mut("text")
            .and_then(|value| value.as_str())
            .map(str::to_string)
        {
            if text.len() > MAX_OUTPUT_BYTES {
                payload["text"] = json!(truncate_utf8(text, MAX_OUTPUT_BYTES));
                truncated = true;
            }
        }
        if let Some(output) = payload
            .pointer("/item/aggregatedOutput")
            .and_then(Value::as_str)
            .map(str::to_string)
        {
            if output.len() > MAX_OUTPUT_BYTES {
                if let Some(item) = payload.get_mut("item").and_then(Value::as_object_mut) {
                    item.insert(
                        "aggregatedOutput".into(),
                        json!(truncate_utf8(output, MAX_OUTPUT_BYTES)),
                    );
                }
                truncated = true;
            }
        }
        let timestamp = now();
        let db = self.db();
        db.execute("INSERT INTO timeline(session_id,kind,payload_json,truncated,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?5)", params![session_id,kind,payload.to_string(),truncated as i64,timestamp]).map_err(|e| e.to_string())?;
        let id = db.last_insert_rowid();
        Ok(TimelineEvent {
            id,
            session_id: session_id.into(),
            kind: kind.into(),
            payload,
            created_at: timestamp.clone(),
            updated_at: timestamp,
            truncated,
        })
    }

    fn append_and_emit(&self, session_id: &str, kind: &str, payload: Value) -> Result<(), String> {
        if matches!(
            kind,
            "message_delta" | "reasoning_delta" | "command_output" | "file_change_delta"
        ) {
            return self.append_stream_delta(session_id, kind, payload);
        }
        self.flush_streams(session_id);
        let event = self.append_timeline(session_id, kind, payload, false)?;
        self.emit("timeline_appended", session_id, json!(event));
        Ok(())
    }

    fn append_stream_delta(
        &self,
        session_id: &str,
        kind: &str,
        payload: Value,
    ) -> Result<(), String> {
        let delta = payload
            .get("delta")
            .or_else(|| payload.get("text"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        let stream_id = payload
            .get("itemId")
            .or_else(|| payload.pointer("/item/id"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        let key = (session_id.to_string(), format!("{kind}:{stream_id}"));
        let mut streams = self.streams.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(buffer) = streams.get_mut(&key) {
            let current = buffer
                .event
                .payload
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let combined = truncate_utf8(format!("{current}{delta}"), MAX_OUTPUT_BYTES);
            buffer.event.payload = json!({"text":combined});
            buffer.event.truncated = current.len() + delta.len() > MAX_OUTPUT_BYTES;
            buffer.event.updated_at = now();
            self.db()
                .execute(
                    "UPDATE timeline SET payload_json=?2,truncated=?3,updated_at=?4 WHERE id=?1",
                    params![
                        buffer.event.id,
                        buffer.event.payload.to_string(),
                        buffer.event.truncated as i64,
                        buffer.event.updated_at
                    ],
                )
                .map_err(|e| e.to_string())?;
            if buffer.last_emit.elapsed() >= Duration::from_millis(50) {
                self.emit("timeline_updated", session_id, json!(buffer.event));
                buffer.last_emit = Instant::now();
                buffer.pending = false;
            } else {
                buffer.pending = true;
            }
            return Ok(());
        }
        let event = self.append_timeline(session_id, kind, json!({"text":delta}), false)?;
        self.emit("timeline_appended", session_id, json!(event));
        streams.insert(
            key,
            StreamBuffer {
                event,
                last_emit: Instant::now(),
                pending: false,
            },
        );
        Ok(())
    }

    fn flush_streams(&self, session_id: &str) {
        let mut streams = self.streams.lock().unwrap_or_else(|p| p.into_inner());
        let keys = streams
            .keys()
            .filter(|(id, _)| id == session_id)
            .cloned()
            .collect::<Vec<_>>();
        for key in keys {
            if let Some(buffer) = streams.remove(&key) {
                if buffer.pending {
                    self.emit("timeline_updated", session_id, json!(buffer.event));
                }
            }
        }
    }

    fn set_status(&self, session_id: &str, status: SessionStatus) -> Result<(), String> {
        self.db()
            .execute(
                "UPDATE sessions SET status=CASE WHEN archived_at IS NULL THEN ?2 ELSE 'archived' END,updated_at=?3 WHERE id=?1",
                params![session_id, status.as_str(), now()],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn update_codex_ids(
        &self,
        session_id: &str,
        thread_id: Option<&str>,
        turn_id: Option<&str>,
    ) -> Result<(), String> {
        self.db().execute("UPDATE sessions SET codex_thread_id=COALESCE(?2,codex_thread_id),active_turn_id=?3,updated_at=?4 WHERE id=?1", params![session_id,thread_id,turn_id,now()]).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn save_approval(
        &self,
        session_id: &str,
        request_id: Value,
        method: &str,
        params_value: Value,
    ) -> Result<Approval, String> {
        let request_id_json = request_id.to_string();
        if let Some(existing) = self.db().query_row(
            "SELECT id,session_id,request_id_json,method,params_json,status,response_json,created_at,resolved_at FROM approvals WHERE session_id=?1 AND request_id_json=?2 AND method=?3 AND status='pending' ORDER BY created_at DESC LIMIT 1",
            params![session_id, request_id_json, method],
            approval_from_row,
        ).optional().map_err(|e| e.to_string())? {
            return Ok(existing);
        }
        let approval = Approval {
            id: Uuid::new_v4().to_string(),
            session_id: session_id.into(),
            request_id,
            method: method.into(),
            params: params_value,
            status: "pending".into(),
            response: None,
            created_at: now(),
            resolved_at: None,
        };
        self.db().execute("INSERT INTO approvals(id,session_id,request_id_json,method,params_json,status,created_at) VALUES(?1,?2,?3,?4,?5,'pending',?6)", params![approval.id,approval.session_id,approval.request_id.to_string(),approval.method,approval.params.to_string(),approval.created_at]).map_err(|e| e.to_string())?;
        self.set_status(session_id, SessionStatus::WaitingApproval)?;
        Ok(approval)
    }

    fn get_approval(&self, id: &str) -> Result<Approval, String> {
        self.db().query_row("SELECT id,session_id,request_id_json,method,params_json,status,response_json,created_at,resolved_at FROM approvals WHERE id=?1", [id], approval_from_row).optional().map_err(|e|e.to_string())?.ok_or_else(||"Approval не найден".into())
    }

    fn pending_approvals(&self) -> Result<Vec<Approval>, String> {
        let db = self.db();
        let mut statement=db.prepare("SELECT id,session_id,request_id_json,method,params_json,status,response_json,created_at,resolved_at FROM approvals WHERE status='pending' ORDER BY created_at").map_err(|e|e.to_string())?;
        let rows = statement
            .query_map([], approval_from_row)
            .map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())
    }

    fn expire_pending_approvals(&self, session_id: &str) -> Result<(), String> {
        let timestamp = now();
        self.db()
            .execute(
                "UPDATE approvals SET status='resolved',response_json=?2,resolved_at=?3 WHERE session_id=?1 AND status='pending'",
                params![session_id, json!({"stale":true,"reason":"app_server_restarted"}).to_string(), timestamp],
            )
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    fn resolve_server_request(&self, session_id: &str, request_id: &Value) -> Result<(), String> {
        let approval_id = self
            .db()
            .query_row(
                "SELECT id FROM approvals WHERE session_id=?1 AND request_id_json=?2 AND status='pending' ORDER BY created_at DESC LIMIT 1",
                params![session_id, request_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        if let Some(approval_id) = approval_id {
            let timestamp = now();
            self.db()
                .execute(
                    "UPDATE approvals SET status='resolved',response_json=?2,resolved_at=?3 WHERE id=?1",
                    params![approval_id, json!({"resolvedByServer":true}).to_string(), timestamp],
                )
                .map_err(|error| error.to_string())?;
            self.emit(
                "approval_resolved",
                session_id,
                json!({"approvalId":approval_id,"resolvedByServer":true}),
            );
        }
        Ok(())
    }
}

impl Drop for AgentsService {
    fn drop(&mut self) {
        let handles = self.runtimes.get_mut().unwrap_or_else(|p| p.into_inner());
        for handle in handles.values() {
            let _ = handle.tx.try_send(AppCommand::Shutdown(None));
        }
    }
}

async fn run_app_server(
    service: Arc<AgentsService>,
    session: Session,
    mut rx: mpsc::Receiver<AppCommand>,
    ready: tokio::sync::oneshot::Sender<Result<(), String>>,
) {
    let resumed = session.codex_thread_id.is_some();
    let result = start_app_server(&service, &session).await;
    let mut startup = match result {
        Ok(parts) => parts,
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };
    let next_id = AtomicI64::new(100);
    let mut pending_turn_requests = HashSet::<i64>::new();
    let active_turn = Arc::new(tokio::sync::Mutex::new(startup.active_turn_id.take()));
    if resumed
        && matches!(
            &startup.status,
            SessionStatus::Completed | SessionStatus::Interrupted | SessionStatus::Failed
        )
    {
        let _ = service.expire_pending_approvals(&session.id);
    }
    for message in startup.buffered.drain(..) {
        handle_app_message(&service, &session.id, &active_turn, message).await;
    }
    if !resumed {
        let request_id = next_id.fetch_add(1, Ordering::Relaxed);
        if send_turn(
            &mut startup.stdin,
            request_id,
            &startup.thread_id,
            &session.prompt,
            session.model.as_deref(),
            &session.mode,
            None,
        )
        .await
        .is_err()
        {
            let _ = service.set_status(&session.id, SessionStatus::Failed);
            return;
        }
        pending_turn_requests.insert(request_id);
    }
    let _ = service.set_status(&session.id, startup.status);
    let active_turn_id = active_turn.lock().await.clone();
    let _ = service.update_codex_ids(
        &session.id,
        Some(&startup.thread_id),
        active_turn_id.as_deref(),
    );
    service.emit(
        "session_updated",
        &session.id,
        json!(service.get_session(&session.id).ok()),
    );
    let _ = ready.send(Ok(()));
    loop {
        tokio::select! {
            command = rx.recv() => match command {
                Some(AppCommand::Send{text}) => {
                    let turn_id=active_turn.lock().await.clone();
                    let request_id=next_id.fetch_add(1,Ordering::Relaxed);
                    if let Err(error)=send_turn(&mut startup.stdin,request_id,&startup.thread_id,&text,session.model.as_deref(),&session.mode,turn_id.as_deref()).await {
                        let _=service.append_and_emit(&session.id,"error",json!({"message":error}));
                    } else {
                        pending_turn_requests.insert(request_id);
                    }
                }
                Some(AppCommand::Interrupt) => {
                    if let Some(turn_id)=active_turn.lock().await.clone() { let _=send_rpc(&mut startup.stdin,next_id.fetch_add(1,Ordering::Relaxed),"turn/interrupt",json!({"threadId":startup.thread_id,"turnId":turn_id})).await; }
                }
                Some(AppCommand::Approval{request_id,result,done}) => { let result=write_json(&mut startup.stdin,&json!({"id":request_id,"result":result})).await; let _=done.send(result); }
                Some(AppCommand::Shutdown(done)) => { let _=startup.child.kill().await; if let Some(done)=done { let _=done.send(()); } break; }
                None => { let _=startup.child.kill().await; break; }
            },
            line = startup.lines.next_line() => match line {
                Ok(Some(line)) => if let Ok(message)=serde_json::from_str::<Value>(&line) {
                    if let Some(id)=message.get("id").and_then(Value::as_i64).filter(|id| pending_turn_requests.remove(id)) {
                        if let Some(error)=message.get("error") {
                            let _=service.set_status(&session.id,SessionStatus::Failed);
                            let _=service.append_and_emit(&session.id,"error",json!({"requestId":id,"message":error}));
                            service.emit("session_updated",&session.id,json!(service.get_session(&session.id).ok()));
                        } else if let Some(turn_id)=message.pointer("/result/turn/id").and_then(Value::as_str) {
                            *active_turn.lock().await=Some(turn_id.to_string());
                            let _=service.update_codex_ids(&session.id,None,Some(turn_id));
                        }
                    }
                    handle_app_message(&service,&session.id,&active_turn,message).await;
                },
                Ok(None)|Err(_) => {
                    let should_fail=service.get_session(&session.id).map(|current| matches!(current.status.as_str(),"starting"|"running"|"waiting_approval")).unwrap_or(true);
                    if should_fail { let _=service.set_status(&session.id,SessionStatus::Failed); service.emit("session_updated",&session.id,json!(service.get_session(&session.id).ok())); }
                    break;
                }
            }
        }
    }
    service.runtimes().remove(&session.id);
}

async fn start_app_server(
    service: &AgentsService,
    session: &Session,
) -> Result<AppServerStartup, String> {
    let mut child = codex_command()
        .args(["app-server", "--stdio"])
        .current_dir(&session.worktree_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("Не удалось запустить Codex CLI: {e}"))?;
    let mut stdin = child.stdin.take().ok_or("Codex stdin недоступен")?;
    let stdout = child.stdout.take().ok_or("Codex stdout недоступен")?;
    let mut lines = BufReader::new(stdout).lines();
    let (init, mut buffered)=rpc_call(&mut stdin,&mut lines,1,"initialize",json!({"clientInfo":{"name":"daedalus","title":"Kosmos Daedalus","version":"0.1.0"},"capabilities":{"experimentalApi":true}})).await?;
    if init.get("error").is_some() {
        return Err(format!("Codex initialize: {}", init["error"]));
    }
    write_json(&mut stdin, &json!({"method":"initialized","params":{}})).await?;
    let (sandbox, approval_policy, reviewer) = mode_params(&session.mode);
    let (method, params_value) = if let Some(thread_id) = &session.codex_thread_id {
        (
            "thread/resume",
            json!({"threadId":thread_id,"cwd":session.worktree_path,"model":session.model,"sandbox":sandbox,"approvalPolicy":approval_policy,"approvalsReviewer":reviewer}),
        )
    } else {
        (
            "thread/start",
            json!({"cwd":session.worktree_path,"model":session.model,"sandbox":sandbox,"approvalPolicy":approval_policy,"approvalsReviewer":reviewer,"experimentalRawEvents":false}),
        )
    };
    let (response, pending) = rpc_call(&mut stdin, &mut lines, 2, method, params_value).await?;
    buffered.extend(pending);
    let thread_id = response
        .pointer("/result/thread/id")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("Codex thread/start не вернул thread id: {response}"))?
        .to_string();
    let last_turn = response
        .pointer("/result/thread/turns")
        .and_then(Value::as_array)
        .and_then(|turns| turns.last());
    let active_turn_id = last_turn
        .filter(|turn| turn.get("status").and_then(Value::as_str) == Some("inProgress"))
        .and_then(|turn| turn.get("id").and_then(Value::as_str))
        .map(str::to_string);
    let status = if session.codex_thread_id.is_none() || active_turn_id.is_some() {
        SessionStatus::Running
    } else {
        match last_turn
            .and_then(|turn| turn.get("status"))
            .and_then(Value::as_str)
        {
            Some("failed") => SessionStatus::Failed,
            Some("interrupted") | Some("inProgress") => SessionStatus::Interrupted,
            _ => SessionStatus::Completed,
        }
    };
    let _ = service.update_codex_ids(&session.id, Some(&thread_id), active_turn_id.as_deref());
    Ok(AppServerStartup {
        child,
        stdin,
        lines,
        thread_id,
        active_turn_id,
        status,
        buffered,
    })
}

async fn handle_app_message(
    service: &AgentsService,
    session_id: &str,
    active_turn: &tokio::sync::Mutex<Option<String>>,
    message: Value,
) {
    if let (Some(id), Some(method)) = (
        message.get("id"),
        message.get("method").and_then(Value::as_str),
    ) {
        let params_value = message.get("params").cloned().unwrap_or_else(|| json!({}));
        if method.contains("requestApproval") || method == "item/tool/requestUserInput" {
            if let Ok(approval) =
                service.save_approval(session_id, id.clone(), method, params_value.clone())
            {
                let kind = if method == "item/tool/requestUserInput" {
                    "question"
                } else {
                    "approval"
                };
                let _ = service.append_and_emit(
                    session_id,
                    kind,
                    json!({"approvalId":approval.id,"method":method,"params":params_value}),
                );
                service.emit("approval_requested", session_id, json!(approval));
            }
            return;
        }
    }
    let method = match message.get("method").and_then(Value::as_str) {
        Some(v) => v,
        None => return,
    };
    let params_value = message.get("params").cloned().unwrap_or_else(|| json!({}));
    if method == "serverRequest/resolved" {
        if let Some(request_id) = params_value.get("requestId") {
            let _ = service.resolve_server_request(session_id, request_id);
        }
        return;
    }
    let kind = match method {
        "turn/started" => {
            if let Some(turn_id) = params_value.pointer("/turn/id").and_then(Value::as_str) {
                *active_turn.lock().await = Some(turn_id.to_string());
                let _ = service.update_codex_ids(session_id, None, Some(turn_id));
            }
            let _ = service.set_status(session_id, SessionStatus::Running);
            "turn_started"
        }
        "turn/completed" => {
            *active_turn.lock().await = None;
            let status = match params_value.pointer("/turn/status").and_then(Value::as_str) {
                Some("failed") => SessionStatus::Failed,
                Some("interrupted") => SessionStatus::Interrupted,
                _ => SessionStatus::Completed,
            };
            let _ = service.set_status(session_id, status);
            let _ = service.update_codex_ids(session_id, None, None);
            "turn_completed"
        }
        "item/started" => "item_started",
        "item/completed" => "item_completed",
        "item/agentMessage/delta" => "message_delta",
        "item/commandExecution/outputDelta" => "command_output",
        "item/fileChange/outputDelta" | "item/fileChange/patchUpdated" => "file_change_delta",
        "item/reasoning/summaryTextDelta" => "reasoning_delta",
        "turn/plan/updated" | "turn/planUpdated" => "plan",
        "turn/diff/updated" => {
            service.emit("diff_updated", session_id, params_value.clone());
            "diff"
        }
        "error" => "error",
        other => {
            tracing::debug!(
                method = other,
                session_id,
                "unknown Codex app-server notification"
            );
            return;
        }
    };
    let _ = service.append_and_emit(session_id, kind, params_value);
    if method == "turn/completed" || method == "turn/started" {
        service.emit(
            "session_updated",
            session_id,
            json!(service.get_session(session_id).ok()),
        );
    }
}

async fn rpc_call(
    stdin: &mut tokio::process::ChildStdin,
    lines: &mut Lines<BufReader<ChildStdout>>,
    id: i64,
    method: &str,
    params_value: Value,
) -> Result<(Value, Vec<Value>), String> {
    send_rpc(stdin, id, method, params_value).await?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(45);
    let mut buffered = Vec::new();
    while let Some(line) = tokio::time::timeout_at(deadline, lines.next_line())
        .await
        .map_err(|_| format!("Codex {method}: timeout"))?
        .map_err(|e| e.to_string())?
    {
        let value: Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
        if value.get("id").and_then(Value::as_i64) == Some(id) {
            return Ok((value, buffered));
        }
        buffered.push(value);
    }
    Err("Codex app-server закрыл stdout".into())
}

async fn send_turn(
    stdin: &mut tokio::process::ChildStdin,
    id: i64,
    thread_id: &str,
    text: &str,
    model: Option<&str>,
    mode: &str,
    active_turn_id: Option<&str>,
) -> Result<(), String> {
    if let Some(turn_id) = active_turn_id {
        return send_rpc(stdin,id,"turn/steer",json!({"threadId":thread_id,"expectedTurnId":turn_id,"input":[{"type":"text","text":text}]})).await;
    }
    let policy = turn_policy(mode);
    send_rpc(
        stdin,
        id,
        "turn/start",
        json!({"threadId":thread_id,"input":[{"type":"text","text":text}],"model":model,"approvalPolicy":policy["approvalPolicy"],"sandboxPolicy":policy["sandboxPolicy"],"approvalsReviewer":policy["approvalsReviewer"]}),
    )
    .await
}

async fn send_rpc(
    stdin: &mut tokio::process::ChildStdin,
    id: i64,
    method: &str,
    params_value: Value,
) -> Result<(), String> {
    write_json(
        stdin,
        &json!({"id":id,"method":method,"params":params_value}),
    )
    .await
}
async fn write_json(stdin: &mut tokio::process::ChildStdin, value: &Value) -> Result<(), String> {
    stdin
        .write_all(format!("{value}\n").as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    stdin.flush().await.map_err(|e| e.to_string())
}

async fn codex_one_shot(method: &str, params_value: Value) -> Result<Value, String> {
    let mut child = codex_command()
        .args(["app-server", "--stdio"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| e.to_string())?;
    let mut stdin = child.stdin.take().ok_or("Codex stdin недоступен")?;
    let stdout = child.stdout.take().ok_or("Codex stdout недоступен")?;
    let mut lines = BufReader::new(stdout).lines();
    let _=rpc_call(&mut stdin,&mut lines,1,"initialize",json!({"clientInfo":{"name":"daedalus","version":"0.1.0"},"capabilities":{"experimentalApi":true}})).await?;
    write_json(&mut stdin, &json!({"method":"initialized","params":{}})).await?;
    let (response, _) = rpc_call(&mut stdin, &mut lines, 2, method, params_value).await?;
    let _ = child.kill().await;
    response
        .get("result")
        .cloned()
        .ok_or_else(|| format!("Codex {method}: {response}"))
}

fn mode_params(mode: &str) -> (&'static str, &'static str, &'static str) {
    match mode {
        "auto-review" => ("workspace-write", "on-request", "auto_review"),
        "full-access" => ("danger-full-access", "never", "user"),
        _ => ("workspace-write", "on-request", "user"),
    }
}
fn turn_policy(mode: &str) -> Value {
    let (sandbox, approval_policy, reviewer) = mode_params(mode);
    let sandbox_policy = if sandbox == "danger-full-access" {
        json!({"type":"dangerFullAccess"})
    } else {
        json!({"type":"workspaceWrite","networkAccess":false})
    };
    json!({
        "approvalPolicy": approval_policy,
        "sandboxPolicy": sandbox_policy,
        "approvalsReviewer": reviewer,
    })
}
fn codex_command() -> Command {
    if std::env::var("KOSMOS_TEST_MODE").as_deref() == Ok("1") {
        if let (Ok(executable), Ok(script)) = (
            std::env::var("DAEDALUS_FAKE_APP_SERVER_EXE"),
            std::env::var("DAEDALUS_FAKE_APP_SERVER_SCRIPT"),
        ) {
            let mut command = Command::new(executable);
            command.arg(script);
            return command;
        }
    }
    Command::new("codex")
}
fn now() -> String {
    Utc::now().to_rfc3339()
}
fn required_str(value: &Value, key: &str) -> Result<String, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|v| !v.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("missing '{key}'"))
}
fn path_str(path: &Path) -> Result<&str, String> {
    path.to_str().ok_or_else(|| "Путь не является UTF-8".into())
}
fn hex_hash(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}
fn prompt_slug(prompt: &str) -> String {
    let slug = prompt
        .chars()
        .flat_map(char::to_lowercase)
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>();
    let compact = slug
        .split('-')
        .filter(|p| !p.is_empty())
        .take(6)
        .collect::<Vec<_>>()
        .join("-");
    if compact.is_empty() {
        "task".into()
    } else {
        compact.chars().take(48).collect()
    }
}
fn truncate_utf8(mut text: String, max: usize) -> String {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
    text
}
fn untracked_patch(path: &str, content: &str) -> String {
    let normalized = path.replace('\\', "/");
    let line_count = content.lines().count().max(1);
    let body = content
        .lines()
        .map(|line| format!("+{line}\n"))
        .collect::<String>();
    format!("\ndiff --git a/{normalized} b/{normalized}\nnew file mode 100644\n--- /dev/null\n+++ b/{normalized}\n@@ -0,0 +1,{line_count} @@\n{body}")
}
fn git_dirty(path: &Path) -> bool {
    std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(path)
        .output()
        .map(|o| !o.stdout.is_empty())
        .unwrap_or(false)
}
fn is_binary(path: &Path) -> bool {
    let mut bytes = [0_u8; 8192];
    std::fs::File::open(path)
        .and_then(|mut file| file.read(&mut bytes))
        .map(|count| bytes[..count].contains(&0))
        .unwrap_or(false)
}

async fn git_output(cwd: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .await
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
async fn git_status(cwd: &Path, args: &[&str]) -> Result<(), String> {
    git_output(cwd, args).await.map(|_| ())
}
async fn command_available(command: &str) -> bool {
    Command::new(command)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false)
}

fn session_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Session> {
    let worktree_path: String = row.get(8)?;
    Ok(Session {
        id: row.get(0)?,
        project_id: row.get(1)?,
        title: row.get(2)?,
        prompt: row.get(3)?,
        mode: row.get(4)?,
        model: row.get(5)?,
        status: row.get(6)?,
        branch: row.get(7)?,
        worktree_exists: Path::new(&worktree_path).is_dir(),
        worktree_path,
        base_commit: row.get(9)?,
        codex_thread_id: row.get(10)?,
        active_turn_id: row.get(11)?,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
        archived_at: row.get(14)?,
    })
}
fn timeline_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TimelineEvent> {
    let raw: String = row.get(3)?;
    Ok(TimelineEvent {
        id: row.get(0)?,
        session_id: row.get(1)?,
        kind: row.get(2)?,
        payload: serde_json::from_str(&raw).unwrap_or(Value::Null),
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
        truncated: row.get::<_, i64>(6)? != 0,
    })
}
fn approval_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Approval> {
    let request: String = row.get(2)?;
    let params_raw: String = row.get(4)?;
    let response: Option<String> = row.get(6)?;
    Ok(Approval {
        id: row.get(0)?,
        session_id: row.get(1)?,
        request_id: serde_json::from_str(&request).unwrap_or(Value::Null),
        method: row.get(3)?,
        params: serde_json::from_str(&params_raw).unwrap_or(Value::Null),
        status: row.get(5)?,
        response: response.and_then(|v| serde_json::from_str(&v).ok()),
        created_at: row.get(7)?,
        resolved_at: row.get(8)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command as StdCommand;
    #[test]
    fn slug_is_git_safe() {
        assert_eq!(prompt_slug(" Fix: login!!! "), "fix-login");
    }
    #[test]
    fn mode_mapping_is_explicit() {
        assert_eq!(
            mode_params("auto-review"),
            ("workspace-write", "on-request", "auto_review")
        );
        assert_eq!(
            mode_params("full-access"),
            ("danger-full-access", "never", "user")
        );
        assert_eq!(
            turn_policy("default")["sandboxPolicy"]["type"],
            "workspaceWrite"
        );
        assert_eq!(
            turn_policy("auto-review")["approvalsReviewer"],
            "auto_review"
        );
        assert_eq!(turn_policy("full-access")["approvalPolicy"], "never");
        assert_eq!(
            turn_policy("full-access")["sandboxPolicy"]["type"],
            "dangerFullAccess"
        );
    }
    #[test]
    fn timeline_paginates() {
        let dir = tempfile::tempdir().unwrap();
        let service = AgentsService::new(dir.path()).unwrap();
        for index in 0..3 {
            service
                .append_timeline("s", "message", json!({"index":index}), false)
                .unwrap();
        }
        let first = service
            .timeline(json!({"session_id":"s","limit":2}))
            .unwrap();
        assert_eq!(first["events"].as_array().unwrap().len(), 2);
        let cursor = first["nextCursor"].as_i64().unwrap();
        let second = service
            .timeline(json!({"session_id":"s","limit":2,"cursor":cursor}))
            .unwrap();
        assert_eq!(second["events"].as_array().unwrap().len(), 1);
    }
    #[test]
    fn output_is_utf8_truncated() {
        let value = "я".repeat(MAX_OUTPUT_BYTES);
        let out = truncate_utf8(value, MAX_OUTPUT_BYTES);
        assert!(out.len() <= MAX_OUTPUT_BYTES);
        assert!(std::str::from_utf8(out.as_bytes()).is_ok());
    }
    #[test]
    fn untracked_text_has_unified_patch() {
        let patch = untracked_patch("src\\new.txt", "one\ntwo\n");
        assert!(patch.contains("+++ b/src/new.txt"));
        assert!(patch.contains("+two"));
    }
    #[test]
    fn persistence_round_trip_keeps_pending_approval() {
        let dir = tempfile::tempdir().unwrap();
        let service = AgentsService::new(dir.path()).unwrap();
        let approval = service
            .save_approval(
                "session",
                json!(7),
                "item/fileChange/requestApproval",
                json!({"reason":"test"}),
            )
            .unwrap();
        drop(service);
        let reopened = AgentsService::new(dir.path()).unwrap();
        let pending = reopened.pending_approvals().unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, approval.id);
    }
    #[tokio::test]
    async fn dirty_base_does_not_leak_into_worktree() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        for args in [
            vec!["init"],
            vec!["config", "user.email", "daedalus@test.invalid"],
            vec!["config", "user.name", "Daedalus Test"],
        ] {
            assert!(StdCommand::new("git")
                .args(args)
                .current_dir(&repo)
                .status()
                .unwrap()
                .success());
        }
        std::fs::write(repo.join("tracked.txt"), "base").unwrap();
        assert!(StdCommand::new("git")
            .args(["add", "tracked.txt"])
            .current_dir(&repo)
            .status()
            .unwrap()
            .success());
        assert!(StdCommand::new("git")
            .args(["commit", "-m", "base"])
            .current_dir(&repo)
            .status()
            .unwrap()
            .success());
        std::fs::write(repo.join("tracked.txt"), "dirty").unwrap();
        let worktree = dir.path().join("worktree");
        git_status(
            &repo,
            &[
                "worktree",
                "add",
                "-b",
                "codex/test-12345678",
                path_str(&worktree).unwrap(),
                "HEAD",
            ],
        )
        .await
        .unwrap();
        let second = dir.path().join("worktree-2");
        git_status(
            &repo,
            &[
                "worktree",
                "add",
                "-b",
                "codex/test-87654321",
                path_str(&second).unwrap(),
                "HEAD",
            ],
        )
        .await
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(worktree.join("tracked.txt")).unwrap(),
            "base"
        );
        assert_eq!(
            std::fs::read_to_string(second.join("tracked.txt")).unwrap(),
            "base"
        );
        let service = AgentsService::new(dir.path()).unwrap();
        let timestamp = now();
        service
            .db()
            .execute(
                "INSERT INTO projects(id,name,path,created_at) VALUES('p','repo',?1,?2)",
                params![repo.to_string_lossy(), timestamp],
            )
            .unwrap();
        service.db().execute(
            "INSERT INTO sessions(id,project_id,title,prompt,mode,status,branch,worktree_path,base_commit,created_at,updated_at,archived_at) VALUES('s','p','t','p','default','archived','codex/test-12345678',?1,'HEAD',?2,?2,?2)",
            params![worktree.to_string_lossy(), timestamp],
        ).unwrap();
        service.remove_worktree("s").await.unwrap();
        assert!(!worktree.exists());
    }

    #[tokio::test]
    async fn app_server_events_are_normalized_and_streams_are_coalesced() {
        let dir = tempfile::tempdir().unwrap();
        let service = AgentsService::new(dir.path()).unwrap();
        let active_turn = tokio::sync::Mutex::new(None);
        for delta in ["Привет, ", "мир"] {
            handle_app_message(
                &service,
                "session",
                &active_turn,
                json!({"method":"item/agentMessage/delta","params":{"delta":delta}}),
            )
            .await;
        }
        handle_app_message(
            &service,
            "session",
            &active_turn,
            json!({"method":"item/completed","params":{"item":{"id":"item-1"}}}),
        )
        .await;
        handle_app_message(
            &service,
            "session",
            &active_turn,
            json!({"id":9,"method":"item/fileChange/requestApproval","params":{"reason":"test"}}),
        )
        .await;
        let page = service
            .timeline(json!({"session_id":"session","limit":20}))
            .unwrap();
        let events = page["events"].as_array().unwrap();
        let message = events
            .iter()
            .find(|event| event["kind"] == "message_delta")
            .unwrap();
        assert_eq!(message["payload"]["text"], "Привет, мир");
        assert_eq!(service.pending_approvals().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn diff_contains_committed_uncommitted_and_untracked_text() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        for args in [
            vec!["init"],
            vec!["config", "user.email", "daedalus@test.invalid"],
            vec!["config", "user.name", "Daedalus Test"],
        ] {
            assert!(StdCommand::new("git")
                .args(args)
                .current_dir(&repo)
                .status()
                .unwrap()
                .success());
        }
        std::fs::write(repo.join("tracked.txt"), "base\n").unwrap();
        assert!(StdCommand::new("git")
            .args(["add", "tracked.txt"])
            .current_dir(&repo)
            .status()
            .unwrap()
            .success());
        assert!(StdCommand::new("git")
            .args(["commit", "-m", "base"])
            .current_dir(&repo)
            .status()
            .unwrap()
            .success());
        let base = git_output(&repo, &["rev-parse", "HEAD"])
            .await
            .unwrap()
            .trim()
            .to_string();
        let worktree = dir.path().join("worktree");
        git_status(
            &repo,
            &[
                "worktree",
                "add",
                "-b",
                "codex/diff-12345678",
                path_str(&worktree).unwrap(),
                &base,
            ],
        )
        .await
        .unwrap();
        std::fs::write(worktree.join("committed.txt"), "committed\n").unwrap();
        git_status(&worktree, &["add", "committed.txt"])
            .await
            .unwrap();
        git_status(&worktree, &["commit", "-m", "agent commit"])
            .await
            .unwrap();
        std::fs::write(worktree.join("tracked.txt"), "working\n").unwrap();
        std::fs::write(worktree.join("untracked.txt"), "untracked\n").unwrap();

        let service = AgentsService::new(dir.path()).unwrap();
        let timestamp = now();
        service.db().execute(
            "INSERT INTO sessions(id,project_id,title,prompt,mode,status,branch,worktree_path,base_commit,created_at,updated_at) VALUES('s','p','t','p','default','running','b',?1,?2,?3,?3)",
            params![worktree.to_string_lossy(),base,timestamp],
        ).unwrap();
        let diff = service.diff("s").await.unwrap();
        let unified = diff["unifiedDiff"].as_str().unwrap();
        assert!(unified.contains("committed"));
        assert!(unified.contains("working"));
        assert!(unified.contains("untracked"));
        service
            .db()
            .execute(
                "UPDATE sessions SET status='archived',archived_at=?2 WHERE id=?1",
                params!["s", now()],
            )
            .unwrap();
        assert!(service.remove_worktree("s").await.is_err());
        assert!(worktree.exists());
    }

    #[tokio::test]
    #[ignore = "requires installed and authenticated Codex CLI"]
    async fn real_codex_smoke_creates_file() {
        assert_eq!(
            std::env::var("DAEDALUS_REAL_CODEX_SMOKE").as_deref(),
            Ok("1"),
            "set DAEDALUS_REAL_CODEX_SMOKE=1 explicitly"
        );
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        for args in [
            vec!["init"],
            vec!["config", "user.email", "daedalus@test.invalid"],
            vec!["config", "user.name", "Daedalus Test"],
        ] {
            assert!(StdCommand::new("git")
                .args(args)
                .current_dir(&repo)
                .status()
                .unwrap()
                .success());
        }
        std::fs::write(repo.join("README.md"), "Daedalus smoke\n").unwrap();
        assert!(StdCommand::new("git")
            .args(["add", "README.md"])
            .current_dir(&repo)
            .status()
            .unwrap()
            .success());
        assert!(StdCommand::new("git")
            .args(["commit", "-m", "base"])
            .current_dir(&repo)
            .status()
            .unwrap()
            .success());

        let service = AgentsService::new(dir.path()).unwrap();
        let project = service.add_project(path_str(&repo).unwrap()).await.unwrap();
        let session = service
            .create_session(json!({
                "project_id": project["id"],
                "prompt": "Создай файл daedalus-real-smoke.txt с единственной строкой ok. Не изменяй другие файлы.",
                "mode": "default"
            }))
            .await
            .unwrap();
        let session_id = session["id"].as_str().unwrap();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(180);
        loop {
            for approval in service.pending_approvals().unwrap() {
                if approval.session_id == session_id {
                    service
                        .respond_approval(json!({"approval_id":approval.id,"decision":"accept"}))
                        .await
                        .unwrap();
                }
            }
            let current = service.get_session(session_id).unwrap();
            if matches!(
                current.status.as_str(),
                "completed" | "failed" | "interrupted"
            ) {
                assert_eq!(current.status, "completed");
                assert_eq!(
                    std::fs::read_to_string(
                        Path::new(&current.worktree_path).join("daedalus-real-smoke.txt")
                    )
                    .unwrap()
                    .trim(),
                    "ok"
                );
                break;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "Codex smoke timed out"
            );
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
        let handles = service
            .runtimes()
            .drain()
            .map(|(_, handle)| handle)
            .collect::<Vec<_>>();
        for handle in handles {
            let (done_tx, done_rx) = tokio::sync::oneshot::channel();
            let _ = handle.tx.send(AppCommand::Shutdown(Some(done_tx))).await;
            let _ = tokio::time::timeout(Duration::from_secs(5), done_rx).await;
        }
    }

    #[tokio::test]
    async fn fake_app_server_recovers_session_and_expires_stale_approval() {
        let script = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/e2e/fixtures/daedalus-fake-app-server.mjs")
            .canonicalize()
            .unwrap();
        std::env::set_var("KOSMOS_TEST_MODE", "1");
        std::env::set_var("DAEDALUS_FAKE_APP_SERVER_EXE", "bun");
        std::env::set_var("DAEDALUS_FAKE_APP_SERVER_SCRIPT", &script);

        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        for args in [
            vec!["init"],
            vec!["config", "user.email", "daedalus@test.invalid"],
            vec!["config", "user.name", "Daedalus Test"],
        ] {
            assert!(StdCommand::new("git")
                .args(args)
                .current_dir(&repo)
                .status()
                .unwrap()
                .success());
        }
        std::fs::write(repo.join("README.md"), "fixture\n").unwrap();
        assert!(StdCommand::new("git")
            .args(["add", "README.md"])
            .current_dir(&repo)
            .status()
            .unwrap()
            .success());
        assert!(StdCommand::new("git")
            .args(["commit", "-m", "base"])
            .current_dir(&repo)
            .status()
            .unwrap()
            .success());

        let service = AgentsService::new(dir.path()).unwrap();
        let project = service.add_project(path_str(&repo).unwrap()).await.unwrap();
        let session = service
            .create_session(
                json!({"project_id":project["id"],"prompt":"fake recovery","mode":"default"}),
            )
            .await
            .unwrap();
        let session_id = session["id"].as_str().unwrap().to_string();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        while service.pending_approvals().unwrap().is_empty() {
            assert!(tokio::time::Instant::now() < deadline);
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        let runtime = { service.runtimes().remove(&session_id).unwrap() };
        let (done_tx, done_rx) = tokio::sync::oneshot::channel();
        runtime
            .tx
            .send(AppCommand::Shutdown(Some(done_tx)))
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(5), done_rx)
            .await
            .unwrap()
            .unwrap();
        drop(service);

        let reopened = AgentsService::new(dir.path()).unwrap();
        reopened.restore_active_sessions().await;
        assert_eq!(
            reopened.get_session(&session_id).unwrap().status,
            "completed"
        );
        assert!(reopened.pending_approvals().unwrap().is_empty());
        reopened.send(&session_id, "follow-up").await.unwrap();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        while reopened.pending_approvals().unwrap().is_empty() {
            assert!(tokio::time::Instant::now() < deadline);
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        reopened.shutdown().await;
        assert!(reopened.runtimes().is_empty());

        std::env::remove_var("KOSMOS_TEST_MODE");
        std::env::remove_var("DAEDALUS_FAKE_APP_SERVER_EXE");
        std::env::remove_var("DAEDALUS_FAKE_APP_SERVER_SCRIPT");
    }
}
