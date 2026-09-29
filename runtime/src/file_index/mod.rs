// File Index v1 — host-local filename/path search for Mundus launcher.
//
// Storage lives in `<data_dir>/file-index.db`, not ARK. File paths are tied to
// this machine and the index can be rebuilt from disk.

mod async_ops;
mod classification;
mod constructors;
mod diagnostics;
mod estimate;
mod risk;
mod scan;
// pub(crate): privileged::ntfs_scan reuses `path_contains_noisy_folder` so the
// service-side pre-filter matches the user-mode scanner exactly.
pub(crate) mod scanner;
mod search;
mod settings;
mod storage;
mod store;
mod types;
mod watcher;

#[cfg(test)]
mod tests;

use globset::{Glob, GlobSet, GlobSetBuilder};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use tokio::sync::Mutex as TokioMutex;

use classification::{
    classify_indexed_file, estimate_ignore_matcher, estimate_index_entry_size_bytes,
    estimate_should_index_path_for_root, now_unix_ms, IndexedFileClass,
};
use estimate::EstimateBudget;
use risk::{assess_estimate_risk, assess_roots_risk, indexed_result_is_visible};
pub use scan::env_flag_enabled;

pub use types::*;
