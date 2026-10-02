// Inject — пишет transcript в clipboard и, если безопасно, вставляет его в
// окно, которое было активно до показа pill.

use thiserror::Error;

use super::config::InjectMode;

#[path = "inject_platform.rs"]
mod platform;

pub use platform::capture_foreground_window;
#[cfg(all(windows, test))]
pub(crate) use platform::paste_shortcut_for_window_class;
#[cfg(windows)]
pub(crate) use platform::PasteShortcut;
pub(crate) use platform::{inject_with_adapter, SystemOsAdapter};

#[derive(Debug, Error)]
pub enum InjectError {
    #[error("clipboard: {0}")]
    Clipboard(#[from] arboard::Error),
    #[error("SendInput failed: injected {injected} of {expected} events")]
    SendInput { injected: u32, expected: u32 },
    #[error("foreground target is missing")]
    MissingTarget,
    #[error("foreground target is stale")]
    StaleTarget,
    #[error("foreground restore failed")]
    ForegroundRestore,
    #[error("foreground target changed")]
    TargetChanged,
}

impl InjectError {
    pub(crate) fn safe_reason(&self) -> &'static str {
        match self {
            Self::Clipboard(_) => "clipboard_write_failed",
            Self::SendInput { .. } => "paste_failed",
            Self::MissingTarget => "missing_target_window",
            Self::StaleTarget => "stale_target_window",
            Self::ForegroundRestore => "foreground_restore_failed",
            Self::TargetChanged => "target_window_changed",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Delivery {
    Pasted,
    ClipboardOnly,
    TextOnly,
    ClipboardFallback { reason: &'static str },
}

impl Delivery {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Pasted => "pasted",
            Self::ClipboardOnly => "clipboard_only",
            Self::TextOnly => "text_only",
            Self::ClipboardFallback { .. } => "clipboard_fallback",
        }
    }

    pub(crate) fn reason(self) -> Option<&'static str> {
        match self {
            Self::ClipboardFallback { reason } => Some(reason),
            Self::Pasted | Self::ClipboardOnly | Self::TextOnly => None,
        }
    }

    pub(crate) fn injected(self) -> bool {
        matches!(self, Self::Pasted)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DeliveryResult {
    pub(crate) delivery: Delivery,
}

pub(crate) trait OsAdapter {
    fn set_clipboard(&mut self, text: &str) -> Result<(), InjectError>;
    fn foreground_window(&mut self) -> Option<isize>;
    fn restore_foreground_window(&mut self, raw: isize) -> Result<(), InjectError>;
    #[cfg(windows)]
    fn window_class_name(&mut self, raw: isize) -> Option<String>;
    #[cfg(windows)]
    fn send_paste(&mut self, shortcut: PasteShortcut) -> Result<(), InjectError>;
}

pub(crate) trait Injector: Send + Sync {
    fn inject(
        &self,
        text: &str,
        mode: InjectMode,
        prev_hwnd: Option<isize>,
    ) -> Result<DeliveryResult, InjectError>;
}

pub(crate) struct SystemInjector;

impl Injector for SystemInjector {
    fn inject(
        &self,
        text: &str,
        mode: InjectMode,
        prev_hwnd: Option<isize>,
    ) -> Result<DeliveryResult, InjectError> {
        #[cfg(test)]
        {
            let _ = (text, prev_hwnd);
            return Ok(DeliveryResult {
                delivery: match mode {
                    InjectMode::ClipboardOnly => Delivery::ClipboardOnly,
                    InjectMode::AutoPaste => Delivery::ClipboardFallback {
                        reason: "test_injector",
                    },
                },
            });
        }

        #[cfg(not(test))]
        {
            let mut adapter = SystemOsAdapter;
            inject_with_adapter(text, mode, prev_hwnd, &mut adapter)
        }
    }
}

/// Blocking impl. Вызывать из `spawn_blocking`. AutoPaste оставляет transcript
/// в clipboard после вставки; ClipboardOnly только записывает его туда.
pub fn inject_blocking(
    text: &str,
    mode: InjectMode,
    prev_hwnd: Option<isize>,
) -> Result<(), InjectError> {
    let mut adapter = SystemOsAdapter;
    inject_with_adapter(text, mode, prev_hwnd, &mut adapter).map(|_| ())
}

#[cfg(test)]
#[path = "inject_tests.rs"]
mod tests;
