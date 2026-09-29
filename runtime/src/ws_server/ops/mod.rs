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
mod app_net;
mod apps;
mod calculator;
mod command;
mod diagnostics;
mod export;
mod file_index;
mod filesystem;
#[cfg(test)]
mod filesystem_tests;
mod games;
mod package;
mod package_helpers;
mod package_legacy;
mod store;
mod system;
mod transport;

pub(super) use app_index::*;
pub(super) use app_net::*;
pub(super) use apps::*;
pub(super) use calculator::*;
pub(super) use command::*;
pub(super) use diagnostics::*;
pub(super) use export::*;
pub(super) use file_index::*;
pub(super) use filesystem::*;
pub(super) use games::*;
pub(super) use package::*;
pub(super) use package_helpers::*;
pub(super) use package_legacy::restore_migration_snapshot;
pub(super) use store::*;
pub(super) use system::*;
pub(super) use transport::*;
