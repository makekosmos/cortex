// Platform dispatch — выбираем источники и launch impl по cfg(target_os).
//
// Windows: Start Menu + UWP.
// macOS: /Applications .app bundles.
// Linux: позже — XdgDesktopSource.

use crate::app_index::app::App;
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
use crate::app_index::AppIndexError;
use crate::app_index::{AppSource, Result};

#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "windows")]
pub fn default_sources(icon_cache_dir: std::path::PathBuf) -> Vec<Box<dyn AppSource>> {
    windows::sources(icon_cache_dir)
}

#[cfg(target_os = "macos")]
pub fn default_sources(_icon_cache_dir: std::path::PathBuf) -> Vec<Box<dyn AppSource>> {
    macos::sources()
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn default_sources(_icon_cache_dir: std::path::PathBuf) -> Vec<Box<dyn AppSource>> {
    Vec::new()
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn launch(_app: &App) -> Result<()> {
    Err(AppIndexError::Launch(
        "launching apps is not implemented for this platform".into(),
    ))
}

#[cfg(target_os = "macos")]
pub fn launch(app: &App) -> Result<()> {
    macos::launch(app)
}

#[cfg(target_os = "windows")]
pub fn launch(app: &App) -> Result<()> {
    windows::launch(app)
}
