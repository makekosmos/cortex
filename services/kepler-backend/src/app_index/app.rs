// App — общая структура одного приложения.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AppKind {
    /// Обычная Win32 / .exe программа.
    Win32,
    /// UWP / Microsoft Store app, запускается через `shell:AppsFolder\<AUMID>`.
    Uwp,
    /// macOS .app bundle (для будущей реализации).
    MacBundle,
    /// Linux .desktop entry (для будущей реализации).
    LinuxDesktop,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct App {
    /// Стабильный id — SHA256(exec_path)[..16] для Win32, AUMID для UWP.
    pub id: String,
    /// Display name (из .lnk filename или Package.DisplayName).
    pub name: String,
    /// Путь к запускаемому файлу (Win32) или `shell:AppsFolder\<AUMID>` (UWP).
    pub exec_path: String,
    /// Путь к PNG иконке в icon cache dir. None — иконка ещё не извлечена.
    pub icon_path: Option<String>,
    pub kind: AppKind,
    /// Имя источника (matches `AppSource::name()`).
    pub source: String,
    /// Mtime exec_path (или Install/PackageMtime для UWP) — для diff/invalidation.
    pub mtime: i64,
}
