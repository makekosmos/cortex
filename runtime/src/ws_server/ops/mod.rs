use super::*;

pub(super) struct LocalResponse {
    pub(super) ok: bool,
    pub(super) data: serde_json::Value,
    pub(super) error: Option<String>,
}

impl LocalResponse {
    pub(super) fn ok(data: serde_json::Value) -> Self {
        Self {
            ok: true,
            data,
            error: None,
        }
    }

    pub(super) fn err(msg: impl Into<String>) -> Self {
        Self {
            ok: false,
            data: serde_json::Value::Null,
            error: Some(msg.into()),
        }
    }
}

mod app_index;
mod command;
mod diagnostics;
mod file_index;
mod system;
mod transport;

pub(super) use app_index::*;
pub(super) use command::*;
pub(super) use diagnostics::*;
pub(super) use file_index::*;
pub(super) use system::*;
pub(super) use transport::*;
