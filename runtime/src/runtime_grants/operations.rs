use super::LaunchGrant;

/// Dictation is an Engine-owned privileged service. Package manifests can only
/// request these named operations; microphone capture itself never crosses the
/// package boundary.
pub const DICTATION_READ_OPERATIONS: &[&str] = &[
    "dictation.get_state",
    "dictation.get_config",
    "dictation.list_local_models",
    "dictation.capture.start",
    "dictation.capture.stop",
    "dictation.speech.transcribe",
    "dictation.input.insert_text",
    "dictation.window.foreground",
];
pub const DICTATION_WRITE_OPERATIONS: &[&str] = &[
    "dictation.update_config",
    "dictation.start_recording",
    "dictation.cancel",
    "dictation.lifecycle.set_autostart",
];
const DICTATION_CONTROL_OPERATIONS: &[&str] = &[
    "dictation.capture.start",
    "dictation.capture.stop",
    "dictation.speech.transcribe",
    "dictation.input.insert_text",
    "dictation.window.foreground",
    "dictation.lifecycle.set_autostart",
];

pub fn dictation_operation_capability(operation: &str) -> Option<&'static str> {
    if DICTATION_CONTROL_OPERATIONS.contains(&operation) {
        Some("dictation.control")
    } else if DICTATION_READ_OPERATIONS.contains(&operation) {
        Some("ark.read")
    } else if DICTATION_WRITE_OPERATIONS.contains(&operation) {
        Some("ark.write")
    } else {
        None
    }
}

/// Focus packages can manage only their own persisted block-lists and timer.
/// Native blocking remains a Host concern and is never an ARK permission.
pub const FOCUS_READ_OPERATIONS: &[&str] = &[
    "focus.list_blocklists",
    "focus.get_active_state",
    "focus.resolve_blocklist_domains",
    "pomodoro.get_state",
];
pub const FOCUS_WRITE_OPERATIONS: &[&str] = &[
    "focus.upsert_blocklist",
    "focus.delete_blocklist",
    "focus.set_active_state",
    "pomodoro.start",
    "pomodoro.pause",
    "pomodoro.resume",
    "pomodoro.skip",
    "pomodoro.stop",
];

pub fn focus_operation_capability(operation: &str) -> Option<&'static str> {
    if FOCUS_READ_OPERATIONS.contains(&operation) {
        Some("ark.read")
    } else if FOCUS_WRITE_OPERATIONS.contains(&operation) {
        Some("ark.write")
    } else {
        None
    }
}

/// Daedalus may use only these exact Engine-owned agent operations. The
/// capability split is intentionally explicit; a package cannot turn a read
/// grant into an agent mutation or use an unregistered `agents.*` operation.
pub const AGENTS_READ_OPERATIONS: &[&str] = &[
    "agents.projects.list",
    "agents.sessions.list",
    "agents.sessions.get",
    "agents.sessions.timeline",
    "agents.diff.get",
    "agents.models.list",
    "agents.editors.list",
    "agents.snapshot",
];
pub const AGENTS_WRITE_OPERATIONS: &[&str] = &[
    "agents.projects.add",
    "agents.projects.remove",
    "agents.sessions.create",
    "agents.sessions.send",
    "agents.sessions.interrupt",
    "agents.sessions.archive",
    "agents.sessions.remove_worktree",
    "agents.approvals.respond",
    "agents.editors.open",
];

pub fn agents_operation_capability(operation: &str) -> Option<&'static str> {
    if AGENTS_READ_OPERATIONS.contains(&operation) {
        Some("ark.read")
    } else if AGENTS_WRITE_OPERATIONS.contains(&operation) {
        Some("ark.write")
    } else {
        None
    }
}

/// App-facing Engine network ops (KOS-152): `network` manifest scopes are
/// named groups — `bookMetadata` and `images` — and each name owns a fixed
/// set of operations. Origin-form network scopes (`https://…`) are worker
/// grants, not app ops, and never land here.
pub const APP_NETWORK_SCOPES: &[&str] = &["bookMetadata", "images"];
const APP_NETWORK_OPERATIONS: &[(&str, &str)] = &[
    ("bookMetadata.lookupIsbn", "bookMetadata"),
    ("bookMetadata.fetchPage", "bookMetadata"),
    ("images.fetch", "images"),
    ("images.dominantColor", "images"),
    ("images.storeCover", "images"),
];

/// Maps a registered app network operation to the `network` scope that owns
/// it; unknown operation names return `None` so `parse_app_rpc` rejects them.
pub fn app_network_operation_scope(operation: &str) -> Option<&'static str> {
    APP_NETWORK_OPERATIONS
        .iter()
        .find(|(name, _)| *name == operation)
        .map(|(_, scope)| *scope)
}

/// Deny registered app network ops whose `network` scope is not granted.
pub fn require_app_network_scope(operation: &str, grant: &LaunchGrant) -> Result<(), &'static str> {
    let scope = app_network_operation_scope(operation).ok_or("unsupported app operation")?;
    if !grant.allows_app_network_scope(scope) {
        return Err("network grant denied");
    }
    Ok(())
}

/// Vault filesystem operations (KOS-155). `filesystem.read` scopes open,
/// scan, read, and close a user-selected directory grant; `filesystem.write`
/// additionally allows registering an export target and writing beneath it.
/// Each operation is Engine-owned — an app never sees raw filesystem access.
pub const FILESYSTEM_READ_OPERATIONS: &[&str] = &[
    "filesystem.vault.open",
    "filesystem.vault.read",
    "filesystem.vault.close",
];
pub const FILESYSTEM_WRITE_OPERATIONS: &[&str] =
    &["filesystem.vault.register", "filesystem.vault.export"];

pub fn filesystem_operation_capability(operation: &str) -> Option<&'static str> {
    if FILESYSTEM_READ_OPERATIONS.contains(&operation) {
        Some("filesystem.read")
    } else if FILESYSTEM_WRITE_OPERATIONS.contains(&operation) {
        Some("filesystem.write")
    } else {
        None
    }
}

/// Capability lookup covering every scoped app-RPC family (dictation, focus,
/// agents, filesystem). Returns the capability id when the operation belongs
/// to a scoped family.
pub fn scoped_capability(operation: &str) -> Option<&'static str> {
    dictation_operation_capability(operation)
        .or_else(|| focus_operation_capability(operation))
        .or_else(|| agents_operation_capability(operation))
        .or_else(|| filesystem_operation_capability(operation))
}

/// `Some(denied)` when `operation` is capability-scoped and the launch grant
/// does not allow it. `filesystem.*` additionally accepts a worker grant
/// since every vault operation runs Engine-side (no raw FS in the app).
pub fn scoped_capability_denied(operation: &str, grant: &LaunchGrant) -> Option<&'static str> {
    if dictation_operation_capability(operation).is_some() {
        return (!grant.allows_dictation_operation(operation)).then_some("dictation grant denied");
    }
    if focus_operation_capability(operation).is_some() {
        return (!grant.allows_focus_operation(operation)).then_some("focus grant denied");
    }
    if agents_operation_capability(operation).is_some() {
        return (!grant.allows_agents_operation(operation)).then_some("agents grant denied");
    }
    if filesystem_operation_capability(operation).is_some() {
        return (!grant.allows_filesystem_operation(operation)
            && !grant.allows_worker_operation(operation))
        .then_some("filesystem grant denied");
    }
    None
}
