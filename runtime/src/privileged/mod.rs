//! Engine privileged operations — one-time admin grant, no helper exes.
//!
//! Capabilities (generic Engine features, not tied to any single feature):
//! * `hosts` — declarative managed blocks in the Windows hosts file;
//! * `ntfs_scan` — fast whole-drive file listing via the MFT;
//! * `protocol` — versioned request/response over the service pipe;
//! * `pipe` / `pipe_server` — client transport + the service accept loop;
//! * `scm` / `service` / `cli` — SCM install/uninstall/status and the
//!   `privileged` subcommands of the Engine binary;
//! * `client` — the user-mode Engine API (`status`, `enable`, hosts sync).
//!
//! Trust model: the user grants admin once (`system.privileged.enable` →
//! `privileged install` via the `runas` verb). The installed service is a
//! stable Engine-exe copy under `%ProgramFiles%\<Brand>\Service` — Engine
//! updates never prompt again because the pipe protocol is backward
//! compatible. See PR for the full threat model.

pub mod brand;
pub mod cli;
pub mod client;
pub mod hosts;
mod hosts_render;
pub mod protocol;
pub mod request_io;

#[cfg(windows)]
mod impersonate;
#[cfg(windows)]
pub mod install_path;
#[cfg(windows)]
pub mod ntfs_scan;
#[cfg(windows)]
pub mod pipe;
#[cfg(windows)]
pub mod pipe_server;
#[cfg(windows)]
pub mod scm;
#[cfg(windows)]
pub mod scm_status;
#[cfg(windows)]
pub mod service;
#[cfg(windows)]
pub mod token;
