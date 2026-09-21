//! Blocking Engine calls live on a worker thread; the UI drains replies on a
//! 100ms poll (same shape as agenda-gpui's store worker).
use serde_json::Value;
use std::sync::mpsc::{Receiver, Sender};

use crate::engine::Engine;

pub enum Command {
    /// POST /v1/rpc — `slot` routes the reply into `ManagerApp::slots`.
    Rpc {
        slot: String,
        op: &'static str,
        params: Value,
    },
    /// GET /v1/<path> status surface.
    Get { slot: String, path: &'static str },
}

pub struct Reply {
    pub slot: String,
    pub result: Result<Value, String>,
}

pub struct Worker {
    pub commands: Sender<Command>,
    pub replies: Receiver<Reply>,
}

impl Worker {
    pub fn start(data_dir: Option<std::path::PathBuf>) -> Self {
        let (commands, requests) = std::sync::mpsc::channel::<Command>();
        let (results, replies) = std::sync::mpsc::channel::<Reply>();
        std::thread::spawn(move || {
            let engine = Engine { data_dir };
            for request in requests {
                let reply = match request {
                    Command::Rpc { slot, op, params } => Reply {
                        slot,
                        result: engine.rpc(op, params),
                    },
                    Command::Get { slot, path } => Reply {
                        slot,
                        result: engine.status(path),
                    },
                };
                if results.send(reply).is_err() {
                    break;
                }
            }
        });
        Self { commands, replies }
    }
}
