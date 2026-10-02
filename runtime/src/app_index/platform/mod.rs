// Platform dispatch — источники и launch по cfg(target_os).
//
// Windows: Start Menu + UWP.
// Any other host, including Linux: an empty source list and an explicit
// launch error. There is no .desktop scanner in this build; callers still
// link these symbols so the app index compiles on Linux.

use crate::app_index::app::App;
#[cfg(not(target_os = "windows"))]
use crate::app_index::AppIndexError;
use crate::app_index::{AppSource, Result};

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "windows")]
pub fn default_sources(icon_cache_dir: std::path::PathBuf) -> Vec<Box<dyn AppSource>> {
    windows::sources(icon_cache_dir)
}

#[cfg(not(target_os = "windows"))]
pub fn default_sources(_icon_cache_dir: std::path::PathBuf) -> Vec<Box<dyn AppSource>> {
    Vec::new()
}

#[cfg(not(target_os = "windows"))]
pub fn launch(_app: &App) -> Result<()> {
    Err(AppIndexError::Launch(
        "launching apps is not implemented for this platform".into(),
    ))
}

#[cfg(target_os = "windows")]
pub fn launch(app: &App) -> Result<()> {
    windows::launch(app)
}
