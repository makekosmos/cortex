// Platform dispatch — выбираем источники и launch impl по cfg(target_os).
//
// Windows: Start Menu + UWP.
// macOS: позже — SpotlightSource.
// Linux: позже — XdgDesktopSource.

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

#[cfg(target_os = "windows")]
pub fn launch(app: &App) -> Result<()> {
    windows::launch(app)
}

#[cfg(not(target_os = "windows"))]
pub fn launch(_app: &App) -> Result<()> {
    Err(AppIndexError::Launch(
        "launching apps is implemented for Windows only in v1".into(),
    ))
}
