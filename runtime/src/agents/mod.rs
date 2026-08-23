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

include!("definitions.rs");
include!("app_server.rs");
include!("tests.rs");
