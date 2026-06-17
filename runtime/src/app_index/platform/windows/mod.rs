// Windows source dispatch.

#![cfg(target_os = "windows")]

pub mod start_menu;
pub mod uwp;

use crate::app_index::app::{App, AppKind};
use crate::app_index::{AppIndexError, AppSource, Result};
use std::path::PathBuf;

pub fn sources(_icon_cache_dir: PathBuf) -> Vec<Box<dyn AppSource>> {
    vec![
        Box::new(start_menu::StartMenuSource),
        Box::new(uwp::UwpSource),
    ]
}

pub fn launch(app: &App) -> Result<()> {
    match app.kind {
        AppKind::Win32 => start_menu::launch_win32(&app.exec_path),
        AppKind::Uwp => uwp::launch_uwp(&app.exec_path),
        _ => Err(AppIndexError::Launch(format!(
            "unsupported AppKind on Windows: {:?}",
            app.kind
        ))),
    }
}
