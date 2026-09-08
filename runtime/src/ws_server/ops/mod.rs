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
mod calculator;
mod command;
mod diagnostics;
mod export;
mod file_index;
mod games;
mod package;
mod package_helpers;
mod package_legacy;
mod store;
mod transport;

pub(super) use app_index::*;
pub(super) use calculator::*;
pub(super) use command::*;
pub(super) use diagnostics::*;
pub(super) use export::*;
pub(super) use file_index::*;
pub(super) use games::*;
pub(super) use package::*;
pub(super) use package_helpers::*;
pub(super) use package_legacy::restore_migration_snapshot;
pub(super) use store::*;
pub(super) use transport::*;
