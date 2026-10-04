//! Typed error for the Daedalus agent runtime.
//!
//! User-facing text stays identical to the former `String` errors: variants
//! are grouped by subsystem (app-server process, consent, sessions, storage)
//! and `Display` is what `handle` flattens back to `String` at the RPC
//! boundary.

#[derive(Debug, thiserror::Error)]
pub enum AgentsError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Storage(#[from] rusqlite::Error),
    #[error("{0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Internal(String),

    #[error("Codex app-server недоступен")]
    AppServerUnavailable,
    #[error("Codex app-server недоступен во время остановки")]
    AppServerUnavailableForStop,
    #[error("Codex app-server закрыл канал остановки")]
    AppServerStopChannelClosed,
    #[error("Codex app-server не подтвердил остановку")]
    AppServerStopNotAcked,
    #[error("Codex app-server закрыл stdout")]
    AppServerStdoutClosed,
    #[error("Codex app-server завершился при запуске")]
    AppServerExitedAtStartup,
    #[error("Codex app-server завершился")]
    AppServerExited,
    #[error("Codex app-server не подтвердил ответ")]
    AppServerResponseNotAcked,
    #[error("app-server pipe closed")]
    AppServerPipeClosed,
    #[error("Codex stdin недоступен")]
    CodexStdinUnavailable,
    #[error("Codex stdout недоступен")]
    CodexStdoutUnavailable,
    #[error("Codex initialize: {0}")]
    CodexInitialize(String),
    #[error("Codex {0}: timeout")]
    CodexTimeout(String),
    #[error("Codex {0}: {1}")]
    CodexRpc(String, String),
    #[error("Codex thread/start не вернул thread id: {0}")]
    CodexThreadStart(String),
    #[error("Не удалось запустить Codex CLI: {0}")]
    CodexCliSpawn(std::io::Error),
    #[error("active turn changed before interrupt")]
    TurnChangedBeforeInterrupt,
    #[error("active turn did not acknowledge interrupt")]
    InterruptNotAcked,
    #[error("interrupt already pending")]
    InterruptPending,

    #[error("full-access consent required")]
    ConsentRequired,
    #[error("full-access consent belongs to another connection")]
    ConsentOtherConnection,
    #[error("full-access consent already approved")]
    ConsentAlreadyApproved,
    #[error("full-access consent token denied")]
    ConsentDenied,
    #[error("full-access consent token expired")]
    ConsentExpired,
    #[error("full-access consent token does not match this operation")]
    ConsentMismatch,
    #[error("full-access consent requires mode full-access")]
    ConsentModeMismatch,
    #[error("full-access consent request expired or not found")]
    ConsentRequestMissing,
    #[error("full-access package_id required")]
    ConsentPackageIdRequired,
    #[error("full-access package_version required")]
    ConsentPackageVersionRequired,
    #[error("desktop authority denied")]
    DesktopAuthorityDenied,
    #[error("missing 'approved'")]
    MissingApproved,
    #[error("Approval уже обработан")]
    ApprovalAlreadyHandled,
    #[error("Approval не найден")]
    ApprovalNotFound,
    #[error("security audit failed: {0}")]
    SecurityAudit(String),

    #[error("Сессия не найдена")]
    SessionNotFound,
    #[error("Сессия Codex не запущена")]
    SessionNotRunning,
    #[error("Сессия останавливается или уже архивирована")]
    SessionStoppingOrArchived,
    #[error("У сессии нет активного хода")]
    SessionNoActiveTurn,
    #[error("Сначала архивируйте сессию")]
    ArchiveSessionFirst,
    #[error("Нельзя удалить worktree активной сессии")]
    ActiveWorktreeRemovalDenied,
    #[error("Worktree содержит незакоммиченные изменения")]
    WorktreeDirty,
    #[error("Нельзя удалить проект с активными сессиями")]
    ProjectHasActiveSessions,
    #[error("Проект не найден")]
    ProjectNotFoundRu,
    #[error("project not found")]
    ProjectNotFound,
    #[error("project was not persisted")]
    ProjectNotPersisted,
    #[error("project path is unavailable: {0}")]
    ProjectPath(std::io::Error),
    #[error("runtime stopped")]
    RuntimeStopped,
    #[error("runtime was replaced during startup")]
    RuntimeReplaced,
    #[error("Неизвестный режим")]
    UnknownMode,
    #[error("Неизвестный редактор")]
    UnknownEditor,
    #[error("Редактор не установлен")]
    EditorMissing,
    #[error("Не удалось открыть редактор: {0}")]
    EditorOpen(std::io::Error),
    #[error("missing '{0}'")]
    MissingParam(String),
    #[error("Путь не является UTF-8")]
    NonUtf8Path,
    #[error("agents.{0}: unknown sub-operation")]
    UnknownOp(String),
    #[error("Git working directory is unavailable: {0}")]
    GitCwdUnavailable(std::io::Error),
    #[error("{0}")]
    Rpc(String),
}

impl AgentsError {
    pub(crate) fn internal(error: impl std::fmt::Display) -> Self {
        Self::Internal(error.to_string())
    }
}
