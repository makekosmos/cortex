//! Product-facing names for the privileged Engine service.
//!
//! Every product-derived string for the service lives here so the KOS-266
//! Kosmos → Mundus rename is a one-line change in this file when the parallel
//! branch lands. Code outside this module must not embed product names.

/// Display brand shown to the user (service description, install dir).
pub const PRODUCT_NAME: &str = "Kosmos";

/// Windows service name. Stable across product releases — renaming this
/// orphan the registered service, so only change it together with an
/// explicit service migration.
pub const SERVICE_NAME: &str = "KosmosPrivilegedSvc";

/// Service display name shown in services.msc.
pub const SERVICE_DISPLAY_NAME: &str = "Kosmos Privileged Service";

/// Service description shown in services.msc.
pub const SERVICE_DESCRIPTION: &str =
    "Privileged operations for the Kosmos Engine: managed hosts-file blocks and fast NTFS \
     indexing. Granted once via UAC; runs only fixed operations requested over a restricted \
     local named pipe.";

/// Directory under %ProgramFiles% holding the stable service binary copy.
/// Admin-only writable — the service binary must never live under a
/// user-writable root (%LOCALAPPDATA% etc.) or it becomes a privilege
/// escalation vector.
pub const SERVICE_DIR: &str = "Kosmos\\Service";

/// File name of the service binary copy. Deliberately different from
/// `kepler-backend.exe` so `taskkill /IM kepler-backend.exe` in the
/// installer/uninstaller never kills the running service.
pub const SERVICE_BINARY_NAME: &str = "kosmos-privileged-service.exe";

/// Named pipe the service listens on.
pub const PIPE_NAME: &str = r"\\.\pipe\kosmos-privileged-service";

/// Services from previous releases that `privileged install`/`uninstall`
/// removes. `KosmosSystemSvc` = the old kepler-focus-svc service;
/// `KeplerFocusSvc` = its pre-rename name.
// MIGRATION(KOS-267): remove after 2026-11-01.
pub const LEGACY_SERVICE_NAMES: &[&str] = &["KosmosSystemSvc", "KeplerFocusSvc"];

/// Pipe names the client still probes after the primary pipe, paired with the
/// service name expected to own each (for server-PID verification), for
/// talking to a not-yet-migrated legacy service. Legacy services speak a
/// subset of the current protocol (they ignore the extra `protocol_version`
/// field).
// MIGRATION(KOS-267): remove after 2026-11-01.
pub const LEGACY_PIPES: &[(&str, &str)] = &[
    (r"\\.\pipe\kosmos-system-service", "KosmosSystemSvc"),
    (r"\\.\pipe\kepler-focus-svc", "KeplerFocusSvc"),
];
