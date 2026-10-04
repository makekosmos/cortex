use super::*;

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
    Interrupting,
    Stopping,
    Completed,
    Interrupted,
    Failed,
    Archived,
}

impl SessionStatus {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Running => "running",
            Self::WaitingApproval => "waiting_approval",
            Self::Interrupting => "interrupting",
            Self::Stopping => "stopping",
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

#[derive(Debug)]
pub(crate) struct RuntimeHandle {
    pub(crate) tx: mpsc::Sender<AppCommand>,
    pub(crate) generation: u64,
    pub(crate) task: tokio::task::JoinHandle<()>,
}

impl RuntimeHandle {
    /// Stopping must end with the task joined: the ack is sent while the task
    /// is still unwinding, and its Arc<AgentsService> — the SQLite connection
    /// inside the data dir — is released only when the task returns. Without
    /// the join a TempDir fixture can outrace the teardown (KOS-314).
    pub(crate) async fn stop(self, session_id: &str) -> Result<(), AgentsError> {
        let (done_tx, done_rx) = tokio::sync::oneshot::channel();
        let result = if self
            .tx
            .send(AppCommand::Shutdown(Some(done_tx)))
            .await
            .is_err()
        {
            // The receiver is gone — the task is already exiting.
            Err(AgentsError::AppServerUnavailableForStop)
        } else {
            match tokio::time::timeout(RUNTIME_ACK_TIMEOUT, done_rx).await {
                Ok(Ok(result)) => result,
                Ok(Err(_)) => Err(AgentsError::AppServerStopChannelClosed),
                Err(_) => Err(AgentsError::AppServerStopNotAcked),
            }
        };
        crate::background_task::join_task(session_id, self.task, RUNTIME_JOIN_TIMEOUT).await;
        result
    }
}

#[derive(Debug, Clone)]
pub(crate) struct StreamBuffer {
    pub(crate) event: TimelineEvent,
    pub(crate) last_emit: Instant,
    pub(crate) pending: bool,
}

pub(crate) struct AppServerStartup {
    pub(crate) process_tree: crate::process_tree::ProcessTree,
    pub(crate) stdin: tokio::process::ChildStdin,
    pub(crate) lines: Lines<BufReader<ChildStdout>>,
    pub(crate) thread_id: String,
    pub(crate) active_turn_id: Option<String>,
    pub(crate) status: SessionStatus,
    pub(crate) buffered: Vec<Value>,
}

#[derive(Debug)]
pub(crate) enum AppCommand {
    Send {
        text: String,
    },
    Interrupt {
        expected_turn_id: String,
        done: tokio::sync::oneshot::Sender<Result<(), AgentsError>>,
    },
    Approval {
        request_id: Value,
        result: Value,
        done: tokio::sync::oneshot::Sender<Result<(), AgentsError>>,
    },
    Shutdown(Option<tokio::sync::oneshot::Sender<Result<(), AgentsError>>>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FullAccessConsentBinding {
    pub(crate) package_id: String,
    pub(crate) package_version: String,
    pub(crate) project_id: String,
    pub(crate) project_path: String,
    pub(crate) mode: String,
    pub(crate) model: Option<String>,
    pub(crate) prompt_hash: String,
    pub(crate) connection_id: Option<u64>,
}

#[derive(Debug, Clone)]
pub(crate) struct FullAccessConsent {
    pub(crate) token: String,
    pub(crate) token_hash: [u8; 32],
    pub(crate) binding: FullAccessConsentBinding,
    pub(crate) expires_at: Instant,
    pub(crate) approved: bool,
}

#[derive(Debug, Default)]
pub(crate) struct FullAccessConsentRegistry {
    pub(crate) pending: HashMap<String, FullAccessConsent>,
}

impl FullAccessConsentRegistry {
    pub(crate) fn issue(&mut self, binding: FullAccessConsentBinding) -> Value {
        self.issue_at(binding, Instant::now())
    }

    pub(crate) fn issue_at(&mut self, binding: FullAccessConsentBinding, now: Instant) -> Value {
        self.purge_at(now);
        let request_id = Uuid::new_v4().to_string();
        let token = new_consent_token();
        let expires_at = now + FULL_ACCESS_CONSENT_TTL;
        let expires_at_rfc3339 = (Utc::now()
            + chrono::Duration::seconds(FULL_ACCESS_CONSENT_TTL.as_secs() as i64))
        .to_rfc3339();
        self.pending.insert(
            request_id.clone(),
            FullAccessConsent {
                token_hash: Sha256::digest(token.as_bytes()).into(),
                token,
                binding: binding.clone(),
                expires_at,
                approved: false,
            },
        );
        json!({
            "request_id": request_id,
            "package_id": binding.package_id,
            "package_version": binding.package_version,
            "project_id": binding.project_id,
            "project_path": binding.project_path,
            "mode": binding.mode,
            "model": binding.model,
            "prompt_sha256": binding.prompt_hash,
            "expires_at": expires_at_rfc3339,
        })
    }

    pub(crate) fn binding(&self, request_id: &str) -> Option<FullAccessConsentBinding> {
        self.pending
            .get(request_id)
            .map(|consent| consent.binding.clone())
    }

    pub(crate) fn approve(
        &mut self,
        request_id: &str,
        approved: bool,
        connection_id: Option<u64>,
    ) -> Result<Option<String>, AgentsError> {
        self.approve_at(request_id, approved, connection_id, Instant::now())
    }

    pub(crate) fn approve_at(
        &mut self,
        request_id: &str,
        approved: bool,
        connection_id: Option<u64>,
        now: Instant,
    ) -> Result<Option<String>, AgentsError> {
        self.purge_at(now);
        let consent = self
            .pending
            .get_mut(request_id)
            .ok_or_else(|| AgentsError::ConsentRequestMissing)?;
        if consent.binding.connection_id != connection_id {
            return Err(AgentsError::ConsentOtherConnection);
        }
        if !approved {
            self.pending.remove(request_id);
            return Ok(None);
        }
        if consent.approved {
            return Err(AgentsError::ConsentAlreadyApproved);
        }
        consent.approved = true;
        Ok(Some(consent.token.clone()))
    }

    pub(crate) fn consume(
        &mut self,
        token: &str,
        binding: &FullAccessConsentBinding,
    ) -> Result<(), AgentsError> {
        self.consume_at(token, binding, Instant::now())
    }

    pub(crate) fn consume_at(
        &mut self,
        token: &str,
        binding: &FullAccessConsentBinding,
        now: Instant,
    ) -> Result<(), AgentsError> {
        let token_hash: [u8; 32] = Sha256::digest(token.as_bytes()).into();
        let request_id = self.pending.iter().find_map(|(request_id, consent)| {
            (constant_time_equal(&consent.token_hash, &token_hash)).then_some(request_id.clone())
        });
        let Some(request_id) = request_id else {
            return Err(AgentsError::ConsentDenied);
        };
        let consent = self
            .pending
            .remove(&request_id)
            .expect("consent found before removal");
        if consent.expires_at <= now {
            return Err(AgentsError::ConsentExpired);
        }
        if !consent.approved || consent.binding != *binding {
            return Err(AgentsError::ConsentMismatch);
        }
        Ok(())
    }

    pub(crate) fn purge_at(&mut self, now: Instant) {
        self.pending.retain(|_, consent| consent.expires_at > now);
    }
}

fn constant_time_equal(left: &[u8; 32], right: &[u8; 32]) -> bool {
    left.iter()
        .zip(right)
        .fold(0u8, |diff, (a, b)| diff | (a ^ b))
        == 0
}

pub struct AgentsService {
    pub(crate) root: PathBuf,
    pub(crate) db: Mutex<Connection>,
    pub(crate) runtimes: Mutex<HashMap<String, RuntimeHandle>>,
    /// Tasks whose handle left `runtimes` without being joined (a task
    /// deregisters itself at the end of run_app_server). Kept so shutdown()
    /// can still wait for their last instructions — the Arc<AgentsService>
    /// they hold keeps the SQLite connection open until the task returns.
    /// Entries are pruned on push, so the vec is bounded by live tasks.
    pub(crate) finished_tasks: crate::background_task::TaskRegistry,
    pub(crate) lifecycle_locks: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    pub(crate) next_runtime_generation: AtomicU64,
    pub(crate) events: broadcast::Sender<Value>,
    pub(crate) seq: AtomicU64,
    pub(crate) event_order: Mutex<()>,
    pub(crate) recent_events: Mutex<VecDeque<Value>>,
    pub(crate) restored: AtomicBool,
    pub(crate) restore_lock: tokio::sync::Mutex<()>,
    pub(crate) streams: Mutex<HashMap<(String, String), StreamBuffer>>,
    pub(crate) full_access_consents: Mutex<FullAccessConsentRegistry>,
}

impl AgentsService {
    pub fn new(data_dir: &Path) -> Result<Arc<Self>, AgentsError> {
        let (events, _) = broadcast::channel(512);
        Self::new_with_events(data_dir, events)
    }

    pub fn new_with_events(
        data_dir: &Path,
        events: broadcast::Sender<Value>,
    ) -> Result<Arc<Self>, AgentsError> {
        let root = data_dir.join("extensions-data").join("daedalus");
        std::fs::create_dir_all(root.join("worktrees")).map_err(AgentsError::internal)?;
        let conn = Connection::open(root.join("daedalus.db")).map_err(AgentsError::internal)?;
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(AgentsError::internal)?;
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
                kind TEXT NOT NULL, payload_json TEXT NOT NULL, truncated INTEGER NOT NULL \
                DEFAULT 0, \
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
             CREATE TABLE IF NOT EXISTS security_audit (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                event TEXT NOT NULL,
                result TEXT NOT NULL,
                package_id TEXT,
                package_version TEXT,
                project_id TEXT,
                project_path TEXT,
                mode TEXT,
                model TEXT,
                prompt_sha256 TEXT,
                created_at TEXT NOT NULL
             );
             INSERT OR IGNORE INTO runtime_meta(key,value) VALUES('event_seq',0);",
        )
        .map_err(AgentsError::internal)?;
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
            finished_tasks: Mutex::new(Vec::new()),
            lifecycle_locks: Mutex::new(HashMap::new()),
            next_runtime_generation: AtomicU64::new(1),
            events,
            seq: AtomicU64::new(initial_seq),
            event_order: Mutex::new(()),
            recent_events: Mutex::new(VecDeque::with_capacity(512)),
            restored: AtomicBool::new(false),
            restore_lock: tokio::sync::Mutex::new(()),
            streams: Mutex::new(HashMap::new()),
            full_access_consents: Mutex::new(FullAccessConsentRegistry::default()),
        }))
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Value> {
        self.events.subscribe()
    }

    pub async fn shutdown(&self) {
        let handles = { self.runtimes().drain().collect::<Vec<_>>() };
        let mut pending = Vec::with_capacity(handles.len());
        for (id, handle) in handles {
            let (done_tx, done_rx) = tokio::sync::oneshot::channel();
            let acked = handle
                .tx
                .send(AppCommand::Shutdown(Some(done_tx)))
                .await
                .is_ok();
            pending.push((id, acked.then_some(done_rx), handle.task));
        }
        for (id, completion, task) in pending {
            if let Some(done_rx) = completion {
                let _ = tokio::time::timeout(RUNTIME_ACK_TIMEOUT, done_rx).await;
            }
            // See RuntimeHandle::stop: the ack precedes the task's teardown,
            // so the join is what makes shutdown deterministic.
            crate::background_task::join_task(&id, task, RUNTIME_JOIN_TIMEOUT).await;
        }
        // Tasks that deregistered themselves keep their Arc until they
        // return; drain them so no connection outlives the service.
        crate::background_task::drain_tasks(&self.finished_tasks, RUNTIME_JOIN_TIMEOUT).await;
    }

    pub(crate) fn db(&self) -> MutexGuard<'_, Connection> {
        self.db
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub(crate) fn runtimes(&self) -> MutexGuard<'_, HashMap<String, RuntimeHandle>> {
        self.runtimes
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub(crate) fn lifecycle_lock(&self, session_id: &str) -> Arc<tokio::sync::Mutex<()>> {
        self.lifecycle_locks
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .entry(session_id.to_string())
            .or_default()
            .clone()
    }

    pub(crate) fn runtime_is_current(&self, session_id: &str, generation: u64) -> bool {
        self.runtimes()
            .get(session_id)
            .is_some_and(|runtime| runtime.generation == generation)
    }

    pub(crate) fn remove_runtime(&self, session_id: &str, generation: u64) {
        let mut runtimes = self.runtimes();
        if runtimes
            .get(session_id)
            .is_some_and(|runtime| runtime.generation == generation)
        {
            if let Some(handle) = runtimes.remove(session_id) {
                crate::background_task::track_task(
                    &self.finished_tasks,
                    format!("agents-runtime:{session_id}"),
                    handle.task,
                );
            }
        }
    }

    pub(crate) fn emit(&self, kind: &str, session_id: &str, payload: Value) {
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

    pub async fn handle(
        self: &Arc<Self>,
        op: &str,
        params: Value,
        client: crate::engine_dispatch::DispatchClient,
    ) -> Result<Value, String> {
        self.handle_typed(op, params, client)
            .await
            .map_err(|error| error.to_string())
    }

    async fn handle_typed(
        self: &Arc<Self>,
        op: &str,
        params: Value,
        client: crate::engine_dispatch::DispatchClient,
    ) -> Result<Value, AgentsError> {
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
            "sessions.issue_full_access_consent" => self.issue_full_access_consent(params, &client),
            "sessions.approve_full_access_consent" | "sessions.respond_full_access_consent" => {
                self.approve_full_access_consent(params, &client)
            }
            "sessions.create" => self.create_session_with_client(params, &client).await,
            "sessions.send" => {
                self.send(
                    &required_str(&params, "session_id")?,
                    &required_str(&params, "text")?,
                )
                .await
            }
            "sessions.interrupt" => self.interrupt(&required_str(&params, "session_id")?).await,
            "sessions.archive" => self.archive(&required_str(&params, "session_id")?).await,
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
            _ => Err(AgentsError::UnknownOp(op.to_string())),
        }
    }

    pub(crate) async fn restore_active_sessions(self: &Arc<Self>) {
        let _guard = self.restore_lock.lock().await;
        if self.restored.load(Ordering::Acquire) {
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
                "starting" | "running" | "waiting_approval" | "interrupting" | "stopping"
            )
        }) {
            let lifecycle = self.lifecycle_lock(&session.id);
            let _session_guard = lifecycle.lock().await;
            if matches!(session.status.as_str(), "interrupting" | "stopping") {
                let _ = self.expire_pending_approvals(&session.id, "runtime_restarted");
                if session.status == "stopping" {
                    let _ = self.mark_archived(&session.id);
                } else {
                    let _ = self.set_status(&session.id, SessionStatus::Interrupted);
                }
                let _ = self.append_and_emit(
                    &session.id,
                    "lifecycle",
                    json!(
                        {"action":"reconcile",
                        "origin":"runtime",
                        "outcome":"forced",
                        "reason":"runtime_restarted"}),
                );
                continue;
            }
            if let Err(error) = self.spawn_runtime(session.clone()).await {
                tracing::warn!(
                    session_id = %session.id,
                    %error,
                    "failed to restore Daedalus session",
                );
                let _ = self.set_status(&session.id, SessionStatus::Failed);
                let _ = self.append_and_emit(
                    &session.id,
                    "lifecycle",
                    json!(
                        {"action":"restore",
                        "origin":"runtime",
                        "outcome":"failed",
                        "reason":"app_server_start_failed"}),
                );
                self.emit(
                    "session_updated",
                    &session.id,
                    json!(self.get_session(&session.id).ok()),
                );
            }
        }
        self.restored.store(true, Ordering::Release);
    }

    pub(crate) fn list_projects(&self) -> Result<Vec<Project>, AgentsError> {
        let db = self.db();
        let mut statement = db
            .prepare("SELECT id,name,path,created_at FROM projects ORDER BY created_at")
            .map_err(AgentsError::internal)?;
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
            .map_err(AgentsError::internal)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(AgentsError::internal)
    }

    pub(crate) async fn add_project(&self, raw_path: &str) -> Result<Value, AgentsError> {
        let requested = PathBuf::from(raw_path);
        let canonical = git_repository_root(&requested)?;
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
            .map_err(AgentsError::internal)?;
        let project = self
            .list_projects()?
            .into_iter()
            .find(|p| p.id == id)
            .ok_or(AgentsError::ProjectNotPersisted)?;
        Ok(json!(project))
    }

    pub(crate) fn remove_project(&self, id: &str) -> Result<Value, AgentsError> {
        let active: i64 = self
            .db()
            .query_row(
                "SELECT COUNT(*) FROM sessions WHERE project_id=?1 AND archived_at IS NULL",
                [id],
                |r| r.get(0),
            )
            .map_err(AgentsError::internal)?;
        if active > 0 {
            return Err(AgentsError::ProjectHasActiveSessions);
        }
        self.db()
            .execute("DELETE FROM projects WHERE id=?1", [id])
            .map_err(AgentsError::internal)?;
        Ok(json!(true))
    }

    pub(crate) fn list_sessions(
        &self,
        include_archived: bool,
    ) -> Result<Vec<Session>, AgentsError> {
        let sql = if include_archived {
            concat!(
                "SELECT id,project_id,title,prompt,mode,model,status,branch,worktree_path,",
                "base_commit,codex_thread_id,active_turn_id,created_at,updated_at,",
                "archived_at FROM sessions ORDER BY updated_at DESC"
            )
        } else {
            concat!(
                "SELECT id,project_id,title,prompt,mode,model,status,branch,worktree_path,",
                "base_commit,codex_thread_id,active_turn_id,created_at,updated_at,",
                "archived_at FROM sessions WHERE archived_at IS NULL ORDER BY updated_at DESC"
            )
        };
        let db = self.db();
        let mut statement = db.prepare(sql).map_err(AgentsError::internal)?;
        let rows = statement
            .query_map([], session_from_row)
            .map_err(AgentsError::internal)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(AgentsError::internal)
    }

    pub(crate) fn get_session(&self, id: &str) -> Result<Session, AgentsError> {
        self.db()
            .query_row(
                concat!(
                    "SELECT id,project_id,title,prompt,mode,model,status,branch,worktree_path,",
                    "base_commit,codex_thread_id,active_turn_id,created_at,updated_at,",
                    "archived_at FROM sessions WHERE id=?1"
                ),
                [id],
                session_from_row,
            )
            .optional()
            .map_err(AgentsError::internal)?
            .ok_or_else(|| AgentsError::SessionNotFound)
    }

    pub(crate) async fn create_session(
        self: &Arc<Self>,
        input: Value,
    ) -> Result<Value, AgentsError> {
        self.create_session_with_client(input, &crate::engine_dispatch::DispatchClient::default())
            .await
    }

    pub(crate) async fn create_session_with_client(
        self: &Arc<Self>,
        input: Value,
        client: &crate::engine_dispatch::DispatchClient,
    ) -> Result<Value, AgentsError> {
        let project_id = required_str(&input, "project_id")?;
        let prompt = required_str(&input, "prompt")?;
        let package_id = input
            .get("package_id")
            .and_then(Value::as_str)
            .map(str::to_string);
        let package_version = input
            .get("package_version")
            .and_then(Value::as_str)
            .map(str::to_string);
        let mode = input
            .get("mode")
            .and_then(Value::as_str)
            .unwrap_or("default")
            .to_string();
        if !matches!(mode.as_str(), "default" | "auto-review" | "full-access") {
            return Err(AgentsError::UnknownMode);
        }
        let project = self
            .list_projects()?
            .into_iter()
            .find(|p| p.id == project_id)
            .ok_or(AgentsError::ProjectNotFoundRu)?;
        let project_path = canonical_project_path(&project)?;
        if mode == "full-access" {
            let nonce = match input
                .get("consent_nonce")
                .and_then(Value::as_str)
                .filter(|value| !value.trim().is_empty())
            {
                Some(nonce) => nonce,
                None => {
                    let binding = FullAccessConsentBinding {
                        package_id: package_id.clone().unwrap_or_default(),
                        package_version: package_version.clone().unwrap_or_default(),
                        project_id: project_id.clone(),
                        project_path: project_path.clone(),
                        mode: mode.clone(),
                        model: input
                            .get("model")
                            .and_then(Value::as_str)
                            .map(str::to_string),
                        prompt_hash: hex_hash(&prompt),
                        connection_id: client.connection_id,
                    };
                    self.audit_security("consent_consume", "denied", Some(&binding))?;
                    return Err(AgentsError::ConsentRequired);
                }
            };
            let expected = FullAccessConsentBinding {
                package_id: package_id.ok_or(AgentsError::ConsentPackageIdRequired)?,
                package_version: package_version
                    .ok_or(AgentsError::ConsentPackageVersionRequired)?,
                project_id: project_id.clone(),
                project_path: project_path.clone(),
                mode: mode.clone(),
                model: input
                    .get("model")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                prompt_hash: hex_hash(&prompt),
                connection_id: client.connection_id,
            };
            let mut consents = self
                .full_access_consents
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if let Err(error) = consents.consume(nonce, &expected) {
                let event = if error.to_string().contains("expired") {
                    "consent_expiry"
                } else if error.to_string().contains("match") {
                    "consent_mismatch"
                } else {
                    "consent_replay"
                };
                self.audit_security(event, "denied", Some(&expected))?;
                return Err(error);
            }
            self.audit_security("consent_consume", "accepted", Some(&expected))?;
        }
        let repo = PathBuf::from(&project_path);
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
            .join(hex_hash(&project_path))
            .join(&id);
        if let Some(parent) = worktree.parent() {
            std::fs::create_dir_all(parent).map_err(AgentsError::internal)?;
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
        self.db()
            .execute(
                "INSERT INTO sessions(id,project_id,title,prompt,mode,model,status,branch,
                 worktree_path,base_commit,created_at,updated_at)
                 VALUES(?1,?2,?3,?4,?5,?6,'starting',?7,?8,?9,?10,?10)",
                params![
                    id,
                    project_id,
                    title,
                    prompt,
                    mode,
                    model,
                    branch,
                    worktree.to_string_lossy(),
                    base_commit,
                    timestamp,
                ],
            )
            .map_err(AgentsError::internal)?;
        self.append_timeline(&id, "user_message", json!({"text": prompt}), false)?;
        let session = self.get_session(&id)?;
        self.emit("session_updated", &id, json!(session));
        if let Err(error) = self.spawn_runtime(session.clone()).await {
            self.set_status(&id, SessionStatus::Failed)?;
            self.append_timeline(&id, "error", json!({"message": error.to_string()}), false)?;
            self.emit("session_updated", &id, json!(self.get_session(&id)?));
            return Err(error);
        }
        Ok(json!(self.get_session(&id)?))
    }

    pub(crate) fn issue_full_access_consent(
        &self,
        input: Value,
        client: &crate::engine_dispatch::DispatchClient,
    ) -> Result<Value, AgentsError> {
        if !client.desktop_authorized {
            self.audit_security("consent_issuance", "denied", None)?;
            return Err(AgentsError::DesktopAuthorityDenied);
        }
        let binding = self.full_access_consent_binding(&input, client)?;
        let result = self
            .full_access_consents
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .issue(binding.clone());
        if let Err(error) = self.audit_security("consent_issuance", "issued", Some(&binding)) {
            if let Some(request_id) = result.get("request_id").and_then(Value::as_str) {
                self.full_access_consents
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .pending
                    .remove(request_id);
            }
            return Err(error);
        }
        Ok(result)
    }

    pub(crate) fn approve_full_access_consent(
        &self,
        input: Value,
        client: &crate::engine_dispatch::DispatchClient,
    ) -> Result<Value, AgentsError> {
        if !client.desktop_authorized {
            self.audit_security("consent_approval", "denied", None)?;
            return Err(AgentsError::DesktopAuthorityDenied);
        }
        let request_id = required_str(&input, "request_id")?;
        let approved = input
            .get("approved")
            .and_then(Value::as_bool)
            .ok_or(AgentsError::MissingApproved)?;
        let mut consents = self
            .full_access_consents
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let binding = consents.binding(&request_id);
        let result = consents.approve(&request_id, approved, client.connection_id);
        drop(consents);
        match result {
            Ok(token) => {
                if let Err(error) = self.audit_security(
                    "consent_approval",
                    if approved { "accepted" } else { "declined" },
                    binding.as_ref(),
                ) {
                    self.full_access_consents
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .pending
                        .remove(&request_id);
                    return Err(error);
                }
                Ok(json!({
                    "consent_nonce": token,
                    "approved": approved,
                }))
            }
            Err(error) => {
                self.audit_security("consent_approval", "denied", binding.as_ref())?;
                Err(error)
            }
        }
    }

    pub(crate) fn full_access_consent_binding(
        &self,
        input: &Value,
        client: &crate::engine_dispatch::DispatchClient,
    ) -> Result<FullAccessConsentBinding, AgentsError> {
        let mode = required_str(input, "mode")?;
        if mode != "full-access" {
            return Err(AgentsError::ConsentModeMismatch);
        }
        let project_id = required_str(input, "project_id")?;
        let project = self
            .list_projects()?
            .into_iter()
            .find(|project| project.id == project_id)
            .ok_or(AgentsError::ProjectNotFound)?;
        Ok(FullAccessConsentBinding {
            package_id: required_str(input, "package_id")?,
            package_version: required_str(input, "package_version")?,
            project_id,
            project_path: canonical_project_path(&project)?,
            mode,
            model: input
                .get("model")
                .and_then(Value::as_str)
                .map(str::to_string),
            prompt_hash: hex_hash(&required_str(input, "prompt")?),
            connection_id: client.connection_id,
        })
    }

    pub(crate) fn audit_security(
        &self,
        event: &str,
        result: &str,
        binding: Option<&FullAccessConsentBinding>,
    ) -> Result<(), AgentsError> {
        self.db()
            .execute(
                concat!(
                    "INSERT INTO security_audit(event,result,package_id,package_version,",
                    "project_id,project_path,mode,model,prompt_sha256,created_at) VALUES(?1,?2,",
                    "?3,?4,?5,?6,?7,?8,?9,?10)"
                ),
                params![
                    event,
                    result,
                    binding.map(|value| value.package_id.as_str()),
                    binding.map(|value| value.package_version.as_str()),
                    binding.map(|value| value.project_id.as_str()),
                    binding.map(|value| value.project_path.as_str()),
                    binding.map(|value| value.mode.as_str()),
                    binding.and_then(|value| value.model.as_deref()),
                    binding.map(|value| value.prompt_hash.as_str()),
                    now(),
                ],
            )
            .map(|_| ())
            .map_err(|error| AgentsError::SecurityAudit(error.to_string()))
    }

    pub(crate) async fn spawn_runtime(
        self: &Arc<Self>,
        session: Session,
    ) -> Result<(), AgentsError> {
        let (tx, rx) = mpsc::channel(64);
        let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
        let generation = self.next_runtime_generation.fetch_add(1, Ordering::Relaxed);
        let service = self.clone();
        let id = session.id.clone();
        {
            // tokio::spawn is synchronous — check, spawn and insert under one
            // lock, so a duplicate registration can never be observed.
            let mut runtimes = self.runtimes();
            if runtimes.contains_key(&id) {
                return Ok(());
            }
            let task = tokio::spawn(async move {
                run_app_server(service, session, generation, rx, ready_tx).await;
            });
            runtimes.insert(
                id.clone(),
                RuntimeHandle {
                    tx,
                    generation,
                    task,
                },
            );
        }
        let result = ready_rx
            .await
            .map_err(|_| AgentsError::AppServerExitedAtStartup)?;
        if result.is_err() {
            self.remove_runtime(&id, generation);
        }
        result
    }

    pub(crate) async fn send(
        self: &Arc<Self>,
        session_id: &str,
        text: &str,
    ) -> Result<Value, AgentsError> {
        let lifecycle = self.lifecycle_lock(session_id);
        let _guard = lifecycle.lock().await;
        let session = self.get_session(session_id)?;
        if matches!(
            session.status.as_str(),
            "interrupting" | "stopping" | "archived"
        ) || session.archived_at.is_some()
        {
            return Err(AgentsError::SessionStoppingOrArchived);
        }
        let mut tx = self.runtimes().get(session_id).map(|h| h.tx.clone());
        if tx.is_none() {
            self.spawn_runtime(session).await?;
            tx = self.runtimes().get(session_id).map(|h| h.tx.clone());
        }
        let tx = tx.ok_or(AgentsError::SessionNotRunning)?;
        let event =
            self.append_timeline(session_id, "user_message", json!({"text": text}), false)?;
        self.emit("timeline_appended", session_id, json!(event));
        tx.send(AppCommand::Send {
            text: text.to_string(),
        })
        .await
        .map_err(|_| AgentsError::AppServerUnavailable)?;
        Ok(json!(true))
    }

    pub(crate) async fn interrupt(&self, session_id: &str) -> Result<Value, AgentsError> {
        let lifecycle = self.lifecycle_lock(session_id);
        let _guard = lifecycle.lock().await;
        let Some(turn_id) = self.get_session(session_id)?.active_turn_id else {
            return Err(AgentsError::SessionNoActiveTurn);
        };
        let tx = self
            .runtimes()
            .get(session_id)
            .map(|h| h.tx.clone())
            .ok_or(AgentsError::SessionNotRunning)?;
        self.set_status(session_id, SessionStatus::Interrupting)?;
        self.emit(
            "session_updated",
            session_id,
            json!(self.get_session(session_id)?),
        );
        let (done_tx, done_rx) = tokio::sync::oneshot::channel();
        if tx
            .send(AppCommand::Interrupt {
                expected_turn_id: turn_id,
                done: done_tx,
            })
            .await
            .is_err()
        {
            self.set_status(session_id, SessionStatus::Failed)?;
            self.append_and_emit(
                session_id,
                "lifecycle",
                json!(
                    {"action":"interrupt",
                    "origin":"user",
                    "outcome":"failed",
                    "reason":"command_channel_closed"}),
            )?;
            self.emit(
                "session_updated",
                session_id,
                json!(self.get_session(session_id)?),
            );
            return Err(AgentsError::AppServerUnavailable);
        }
        let (acknowledged, reason) =
            match tokio::time::timeout(INTERRUPT_ACK_TIMEOUT, done_rx).await {
                Ok(Ok(Ok(()))) => (true, "app_server_acknowledged"),
                Ok(Ok(Err(_))) => (false, "app_server_refused"),
                Ok(Err(_)) => (false, "ack_channel_closed"),
                Err(_) => (false, "ack_timeout"),
            };
        if !acknowledged {
            if let Err(error) = self.stop_runtime(session_id).await {
                self.set_status(session_id, SessionStatus::Failed)?;
                self.append_and_emit(
                    session_id,
                    "lifecycle",
                    json!(
                        {"action":"interrupt",
                        "origin":"user",
                        "outcome":"failed",
                        "reason":"force_stop_failed"}),
                )?;
                self.emit(
                    "session_updated",
                    session_id,
                    json!(self.get_session(session_id)?),
                );
                return Err(error);
            }
            self.set_status(session_id, SessionStatus::Interrupted)?;
            self.update_codex_ids(session_id, None, None)?;
        }
        self.expire_pending_approvals(session_id, "session_interrupted")?;
        self.append_and_emit(
            session_id,
            "lifecycle",
            json!(
                {"action":"interrupt",
                "origin":"user",
                "outcome":if acknowledged {"acknowledged"} else {"forced"},
                "reason":reason}),
        )?;
        self.emit(
            "session_updated",
            session_id,
            json!(self.get_session(session_id)?),
        );
        Ok(json!(true))
    }

    pub(crate) async fn archive(&self, session_id: &str) -> Result<Value, AgentsError> {
        let lifecycle = self.lifecycle_lock(session_id);
        let _guard = lifecycle.lock().await;
        if self.get_session(session_id)?.archived_at.is_some() {
            return Ok(json!(true));
        }
        self.set_status(session_id, SessionStatus::Stopping)?;
        self.emit(
            "session_updated",
            session_id,
            json!(self.get_session(session_id)?),
        );
        if let Err(error) = self.stop_runtime(session_id).await {
            self.set_status(session_id, SessionStatus::Failed)?;
            self.append_and_emit(
                session_id,
                "lifecycle",
                json!(
                    {"action":"archive",
                    "origin":"user",
                    "outcome":"failed",
                    "reason":"force_stop_failed"}),
            )?;
            self.emit(
                "session_updated",
                session_id,
                json!(self.get_session(session_id)?),
            );
            return Err(error);
        }
        self.expire_pending_approvals(session_id, "session_archived")?;
        self.mark_archived(session_id)?;
        self.append_and_emit(
            session_id,
            "lifecycle",
            json!({"action":"archive","origin":"user","outcome":"stopped"}),
        )?;
        self.emit(
            "session_updated",
            session_id,
            json!(self.get_session(session_id)?),
        );
        Ok(json!(true))
    }

    pub(crate) fn mark_archived(&self, session_id: &str) -> Result<(), AgentsError> {
        let timestamp = now();
        self.db()
            .execute(
                concat!(
                    "UPDATE sessions SET status='archived',active_turn_id=NULL,archived_at=?2,",
                    "updated_at=?2 WHERE id=?1"
                ),
                params![session_id, timestamp],
            )
            .map_err(AgentsError::internal)?;
        Ok(())
    }

    pub(crate) async fn stop_runtime(&self, session_id: &str) -> Result<(), AgentsError> {
        let Some(handle) = self.runtimes().remove(session_id) else {
            return Ok(());
        };
        handle.stop(session_id).await
    }

    pub(crate) async fn remove_worktree(&self, session_id: &str) -> Result<Value, AgentsError> {
        let lifecycle = self.lifecycle_lock(session_id);
        let _guard = lifecycle.lock().await;
        let session = self.get_session(session_id)?;
        if session.archived_at.is_none() {
            return Err(AgentsError::ArchiveSessionFirst);
        }
        if session.active_turn_id.is_some() {
            return Err(AgentsError::ActiveWorktreeRemovalDenied);
        }
        if !session.worktree_exists {
            return Ok(json!(true));
        }
        if git_dirty(Path::new(&session.worktree_path)) {
            return Err(AgentsError::WorktreeDirty);
        }
        self.stop_runtime(session_id).await?;
        let project = self
            .list_projects()?
            .into_iter()
            .find(|project| project.id == session.project_id)
            .ok_or(AgentsError::ProjectNotFoundRu)?;
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

    pub(crate) fn timeline(&self, input: Value) -> Result<Value, AgentsError> {
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
        let mut statement = db
            .prepare(concat!(
                "SELECT id,session_id,kind,payload_json,created_at,updated_at,truncated FROM ",
                "timeline WHERE session_id=?1 AND id<?2 ORDER BY id DESC LIMIT ?3"
            ))
            .map_err(AgentsError::internal)?;
        let rows = statement
            .query_map(params![session_id, cursor, limit], timeline_from_row)
            .map_err(AgentsError::internal)?;
        let mut events = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(AgentsError::internal)?;
        events.reverse();
        let next_cursor = events.first().map(|event| event.id);
        Ok(json!({"events":events,"nextCursor":next_cursor}))
    }

    pub(crate) async fn respond_approval(&self, input: Value) -> Result<Value, AgentsError> {
        let approval_id = required_str(&input, "approval_id")?;
        let decision = input
            .get("decision")
            .cloned()
            .unwrap_or_else(|| json!("decline"));
        let approval = self.get_approval(&approval_id)?;
        let lifecycle = self.lifecycle_lock(&approval.session_id);
        let _guard = lifecycle.lock().await;
        let approval = self.get_approval(&approval_id)?;
        if approval.status != "pending" {
            return Err(AgentsError::ApprovalAlreadyHandled);
        }
        if matches!(
            self.get_session(&approval.session_id)?.status.as_str(),
            "interrupting" | "stopping" | "archived"
        ) {
            return Err(AgentsError::SessionStoppingOrArchived);
        }
        let result = if approval.method == "item/tool/requestUserInput" {
            json!({"answers": input.get("answers").cloned().unwrap_or_else(|| json!({}))})
        } else if approval.method == "item/permissions/requestApproval" {
            if decision == json!("accept") {
                json!(
                    {"permissions": approval.params.get(
                        "permissions"
                    ).cloned().unwrap_or_else(|| json!({})),
                    "scope":"turn"})
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
            .ok_or(AgentsError::SessionNotRunning)?;
        let (done_tx, done_rx) = tokio::sync::oneshot::channel();
        tx.send(AppCommand::Approval {
            request_id: approval.request_id.clone(),
            result: result.clone(),
            done: done_tx,
        })
        .await
        .map_err(|_| AgentsError::AppServerUnavailable)?;
        tokio::time::timeout(Duration::from_secs(5), done_rx)
            .await
            .map_err(|_| AgentsError::AppServerResponseNotAcked)?
            .map_err(|_| AgentsError::AppServerExited)??;
        let timestamp = now();
        self.db()
            .execute(
                concat!(
                    "UPDATE approvals SET status='resolved',response_json=?2,resolved_at=?3 ",
                    "WHERE id=?1"
                ),
                params![approval_id, result.to_string(), timestamp],
            )
            .map_err(AgentsError::internal)?;
        let remaining: i64 = self
            .db()
            .query_row(
                "SELECT COUNT(*) FROM approvals WHERE session_id=?1 AND status='pending'",
                [&approval.session_id],
                |row| row.get(0),
            )
            .map_err(AgentsError::internal)?;
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

    pub(crate) async fn diff(&self, session_id: &str) -> Result<Value, AgentsError> {
        let session = self.get_session(session_id)?;
        let cwd = PathBuf::from(&session.worktree_path);
        let canonical_cwd = std::fs::canonicalize(&cwd).map_err(AgentsError::internal)?;
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
            files.push(json!(
                {"path":path,
                "status":state,
                "size":size,
                "binary":binary,
                "contentOmitted":binary || size > MAX_OUTPUT_BYTES as u64}));
        }
        let truncated = unified.len() > MAX_OUTPUT_BYTES;
        let diff = truncate_utf8(unified, MAX_OUTPUT_BYTES);
        Ok(json!(
            {"sessionId":session_id,
            "baseCommit":session.base_commit,
            "files":files,
            "unifiedDiff":diff,
            "truncated":truncated}))
    }

    pub(crate) async fn models(&self) -> Result<Value, AgentsError> {
        let output = codex_one_shot("model/list", json!({"limit":100})).await?;
        Ok(output)
    }

    pub(crate) async fn editors(&self) -> Result<Value, AgentsError> {
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

    pub(crate) async fn open_editor(&self, input: Value) -> Result<Value, AgentsError> {
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
            _ => return Err(AgentsError::UnknownEditor),
        };
        if editor != "explorer" && !command_available(executable).await {
            return Err(AgentsError::EditorMissing);
        }
        let mut command = Command::new(executable);
        command
            .arg(&session.worktree_path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| AgentsError::EditorOpen(e))?;
        Ok(json!(true))
    }

    pub(crate) fn snapshot(&self, input: Value) -> Result<Value, AgentsError> {
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
        Ok(json!(
            {"seq":self.seq.load(Ordering::Relaxed),
            "afterSeq":after_seq,
            "recentEvents":recent_events,
            "projects":self.list_projects()?,
            "sessions":self.list_sessions(false)?,
            "approvals":approvals}))
    }

    pub(crate) fn append_timeline(
        &self,
        session_id: &str,
        kind: &str,
        mut payload: Value,
        mut truncated: bool,
    ) -> Result<TimelineEvent, AgentsError> {
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
        db.execute(
            concat!(
                "INSERT INTO timeline(session_id,kind,payload_json,truncated,created_at,",
                "updated_at) VALUES(?1,?2,?3,?4,?5,?5)"
            ),
            params![
                session_id,
                kind,
                payload.to_string(),
                truncated as i64,
                timestamp
            ],
        )
        .map_err(AgentsError::internal)?;
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

    pub(crate) fn append_and_emit(
        &self,
        session_id: &str,
        kind: &str,
        payload: Value,
    ) -> Result<(), AgentsError> {
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

    pub(crate) fn append_stream_delta(
        &self,
        session_id: &str,
        kind: &str,
        payload: Value,
    ) -> Result<(), AgentsError> {
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
                .map_err(AgentsError::internal)?;
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

    pub(crate) fn flush_streams(&self, session_id: &str) {
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

    pub(crate) fn set_status(
        &self,
        session_id: &str,
        status: SessionStatus,
    ) -> Result<(), AgentsError> {
        self.db()
            .execute(
                concat!(
                    "UPDATE sessions SET status=CASE WHEN archived_at IS NULL THEN ?2 ELSE ",
                    "'archived' END,updated_at=?3 WHERE id=?1"
                ),
                params![session_id, status.as_str(), now()],
            )
            .map_err(AgentsError::internal)?;
        Ok(())
    }

    pub(crate) fn update_codex_ids(
        &self,
        session_id: &str,
        thread_id: Option<&str>,
        turn_id: Option<&str>,
    ) -> Result<(), AgentsError> {
        self.db()
            .execute(
                concat!(
                    "UPDATE sessions SET codex_thread_id=COALESCE(?2,codex_thread_id),",
                    "active_turn_id=?3,updated_at=?4 WHERE id=?1"
                ),
                params![session_id, thread_id, turn_id, now()],
            )
            .map_err(AgentsError::internal)?;
        Ok(())
    }

    pub(crate) fn save_approval(
        &self,
        session_id: &str,
        request_id: Value,
        method: &str,
        params_value: Value,
    ) -> Result<Approval, AgentsError> {
        let request_id_json = request_id.to_string();
        if let Some(existing) = self
            .db()
            .query_row(
                concat!(
                    "SELECT id,session_id,request_id_json,method,params_json,status,",
                    "response_json,created_at,resolved_at FROM approvals WHERE session_id=?1 AND ",
                    "request_id_json=?2 AND method=?3 AND status='pending' ORDER BY created_at ",
                    "DESC LIMIT 1"
                ),
                params![session_id, request_id_json, method],
                approval_from_row,
            )
            .optional()
            .map_err(AgentsError::internal)?
        {
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
        self.db()
            .execute(
                concat!(
                    "INSERT INTO approvals(id,session_id,request_id_json,method,params_json,",
                    "status,created_at) VALUES(?1,?2,?3,?4,?5,'pending',?6)"
                ),
                params![
                    approval.id,
                    approval.session_id,
                    approval.request_id.to_string(),
                    approval.method,
                    approval.params.to_string(),
                    approval.created_at
                ],
            )
            .map_err(AgentsError::internal)?;
        self.set_status(session_id, SessionStatus::WaitingApproval)?;
        Ok(approval)
    }

    pub(crate) fn get_approval(&self, id: &str) -> Result<Approval, AgentsError> {
        self.db()
            .query_row(
                concat!(
                    "SELECT id,session_id,request_id_json,method,params_json,status,",
                    "response_json,created_at,resolved_at FROM approvals WHERE id=?1"
                ),
                [id],
                approval_from_row,
            )
            .optional()
            .map_err(AgentsError::internal)?
            .ok_or_else(|| AgentsError::ApprovalNotFound)
    }

    pub(crate) fn pending_approvals(&self) -> Result<Vec<Approval>, AgentsError> {
        let db = self.db();
        let mut statement = db
            .prepare(concat!(
                "SELECT id,session_id,request_id_json,method,params_json,status,",
                "response_json,created_at,resolved_at FROM approvals WHERE status='pending' ",
                "ORDER BY created_at"
            ))
            .map_err(AgentsError::internal)?;
        let rows = statement
            .query_map([], approval_from_row)
            .map_err(AgentsError::internal)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(AgentsError::internal)
    }

    pub(crate) fn expire_pending_approvals(
        &self,
        session_id: &str,
        reason: &str,
    ) -> Result<(), AgentsError> {
        let timestamp = now();
        let approval_ids = {
            let db = self.db();
            let mut statement = db
                .prepare("SELECT id FROM approvals WHERE session_id=?1 AND status='pending'")
                .map_err(AgentsError::internal)?;
            let ids = statement
                .query_map([session_id], |row| row.get::<_, String>(0))
                .map_err(AgentsError::internal)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(AgentsError::internal)?;
            ids
        };
        self.db()
            .execute(
                concat!(
                    "UPDATE approvals SET status='resolved',response_json=?2,resolved_at=?3 ",
                    "WHERE session_id=?1 AND status='pending'"
                ),
                params![
                    session_id,
                    json!({"stale":true,"reason":reason}).to_string(),
                    timestamp
                ],
            )
            .map_err(AgentsError::internal)?;
        if !approval_ids.is_empty() {
            for approval_id in &approval_ids {
                self.emit(
                    "approval_resolved",
                    session_id,
                    json!({"approvalId":approval_id,"expired":true,"reason":reason}),
                );
            }
            self.emit(
                "approval_resolved",
                session_id,
                json!({"expired":approval_ids.len(),"reason":reason}),
            );
            self.append_and_emit(
                session_id,
                "approval_expired",
                json!({"count":approval_ids.len(),"reason":reason}),
            )?;
        }
        Ok(())
    }

    pub(crate) fn resolve_server_request(
        &self,
        session_id: &str,
        request_id: &Value,
    ) -> Result<(), AgentsError> {
        let approval_id = self
            .db()
            .query_row(
                concat!(
                    "SELECT id FROM approvals WHERE session_id=?1 AND request_id_json=?2 AND ",
                    "status='pending' ORDER BY created_at DESC LIMIT 1"
                ),
                params![session_id, request_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(AgentsError::internal)?;
        if let Some(approval_id) = approval_id {
            let timestamp = now();
            self.db()
                .execute(
                    concat!(
                        "UPDATE approvals SET status='resolved',response_json=?2,resolved_at=?3 ",
                        "WHERE id=?1"
                    ),
                    params![
                        approval_id,
                        json!({"resolvedByServer":true}).to_string(),
                        timestamp
                    ],
                )
                .map_err(AgentsError::internal)?;
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
