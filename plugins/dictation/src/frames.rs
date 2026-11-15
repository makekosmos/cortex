//! The Engine's frame feed.
//!
//! The Engine steps the dot scope and broadcasts every step as
//! `dictation_pill_frame`, so the pill subscribes instead of asking: one
//! WebSocket, one thread, and the newest frame in a slot the UI thread reads.
//!
//! Nothing here draws. It keeps one fact for the window — the newest frame —
//! and reconnects on its own when the Engine restarts under a new port. The
//! process stays warm between dictations: booting it on the first frame would
//! show the pill a second late, after the person already started talking.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::Value;
use tungstenite::Message;

/// How often a missing or restarted Engine is retried.
const RETRY: Duration = Duration::from_millis(500);
/// A gap this long between frames means a session ended: the next frame starts
/// a new one, and is worth a timestamp in the log.
const QUIET: Duration = Duration::from_millis(400);

/// What the UI thread reads: the newest frame.
#[derive(Clone)]
pub struct Frames {
    latest: Arc<Mutex<Option<Value>>>,
    /// When the last frame arrived, so a gap can be told from a stream.
    heard: Arc<Mutex<Option<Instant>>>,
}

impl Frames {
    /// The lock file the Engine publishes: `%APPDATA%\Mundus\engine.lock.json`.
    pub fn lock_dir() -> Option<PathBuf> {
        if let Some(dir) = std::env::var_os("MUNDUS_DATA_DIR") {
            return Some(PathBuf::from(dir));
        }
        std::env::var_os("APPDATA").map(|appdata| PathBuf::from(appdata).join("Mundus"))
    }

    fn new() -> Self {
        Self {
            latest: Arc::new(Mutex::new(None)),
            heard: Arc::new(Mutex::new(None)),
        }
    }

    /// Starts the feed on its own thread and returns the handle the window reads.
    pub fn start() -> Self {
        let frames = Self::new();
        let pump = frames.clone();
        std::thread::spawn(move || pump.run());
        frames
    }

    /// The newest frame, or `None` when the scope has settled and the pill is
    /// meant to be gone.
    pub fn take(&self) -> Option<Value> {
        self.latest
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
    }

    fn store(&self, frame: Value) {
        // The first frame after a quiet stretch is a session start: stamping it is
        // what tells "the Engine was late" from "the window was late".
        let now = Instant::now();
        {
            let mut heard = self.heard.lock().unwrap_or_else(|error| error.into_inner());
            let quiet = heard.is_none_or(|at| now.duration_since(at) > QUIET);
            if quiet {
                crate::hud::log("first frame after quiet");
            }
            *heard = Some(now);
        }
        *self
            .latest
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(frame);
    }

    fn run(&self) {
        loop {
            if let Err(error) = self.connect() {
                eprintln!("dictation: {error}");
            }
            std::thread::sleep(RETRY);
        }
    }

    /// One WebSocket session: the handshake, then every frame until it breaks.
    fn connect(&self) -> Result<(), String> {
        let dir = Self::lock_dir().ok_or("no %APPDATA%")?;
        let bytes = std::fs::read(dir.join("engine.lock.json"))
            .map_err(|error| format!("engine.lock.json unreadable: {error}"))?;
        let lock: Value = serde_json::from_slice(&bytes)
            .map_err(|error| format!("engine.lock.json malformed: {error}"))?;
        let port = lock
            .get("ws_port")
            .and_then(Value::as_u64)
            .ok_or("engine.lock.json has no ws_port")?;
        let token = lock
            .get("auth_token")
            .and_then(Value::as_str)
            .ok_or("engine.lock.json has no auth_token")?;
        let url = format!("ws://127.0.0.1:{port}");
        let (mut socket, _response) =
            tungstenite::connect(url.as_str()).map_err(|error| format!("ws connect: {error}"))?;
        let hello = serde_json::json!({
            "kind": "hello",
            "apiVersion": "1.0.0",
            "token": token,
            "pid": std::process::id(),
            "clientClass": "dictation",
            "clientVersion": env!("CARGO_PKG_VERSION"),
        });
        socket
            .send(Message::Text(hello.to_string().into()))
            .map_err(|error| format!("ws hello: {error}"))?;
        loop {
            let message = socket
                .read()
                .map_err(|error| format!("ws read: {error}"))?;
            let Message::Text(text) = message else {
                continue;
            };
            let Ok(event) = serde_json::from_str::<Value>(&text) else {
                continue;
            };
            if event.get("kind").and_then(Value::as_str) == Some("hello_ok") {
                continue;
            }
            if event.get("event").and_then(Value::as_str) != Some("dictation_pill_frame") {
                continue;
            }
            self.store(event);
        }
    }
}
