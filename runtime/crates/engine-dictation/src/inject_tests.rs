use super::*;

#[derive(Default)]
struct FakeAdapter {
    clipboard_calls: usize,
    restore_calls: usize,
    paste_calls: usize,
    foreground: Option<isize>,
    restore_fails: bool,
    restore_stale: bool,
    #[cfg(target_os = "macos")]
    paste_fails: bool,
}

impl OsAdapter for FakeAdapter {
    fn set_clipboard(&mut self, _text: &str) -> Result<(), InjectError> {
        self.clipboard_calls += 1;
        Ok(())
    }

    fn foreground_window(&mut self) -> Option<isize> {
        self.foreground
    }

    fn restore_foreground_window(&mut self, _raw: isize) -> Result<(), InjectError> {
        self.restore_calls += 1;
        if self.restore_fails {
            Err(if self.restore_stale {
                InjectError::StaleTarget
            } else {
                InjectError::ForegroundRestore
            })
        } else {
            Ok(())
        }
    }

    #[cfg(windows)]
    fn window_class_name(&mut self, _raw: isize) -> Option<String> {
        None
    }

    #[cfg(windows)]
    fn send_paste(&mut self, _shortcut: PasteShortcut) -> Result<(), InjectError> {
        self.paste_calls += 1;
        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn send_paste(&mut self) -> Result<(), InjectError> {
        self.paste_calls += 1;
        if self.paste_fails {
            Err(InjectError::SendInput {
                injected: 0,
                expected: 1,
            })
        } else {
            Ok(())
        }
    }
}

#[test]
fn capture_returns_some_or_none_without_panic() {
    // Не assert'им конкретное значение — зависит от среды (CI без UI = None).
    let _ = capture_foreground_window();
}

#[test]
fn clipboard_only_never_restores_or_sends_input() {
    let mut adapter = FakeAdapter::default();
    let result =
        inject_with_adapter("hello", InjectMode::ClipboardOnly, Some(7), &mut adapter).unwrap();

    assert_eq!(result.delivery, Delivery::ClipboardOnly);
    assert_eq!(adapter.clipboard_calls, 1);
    assert_eq!(adapter.restore_calls, 0);
    assert_eq!(adapter.paste_calls, 0);
}

// The foreground-restore flow below only exists on Windows — on macOS the
// OS restores focus when the pill hides and inject sends Cmd+V straight.
#[cfg(windows)]
#[test]
fn missing_target_falls_back_without_restoring_or_sending_input() {
    let mut adapter = FakeAdapter::default();
    let result = inject_with_adapter("hello", InjectMode::AutoPaste, None, &mut adapter).unwrap();

    assert_eq!(
        result.delivery,
        Delivery::ClipboardFallback {
            reason: "missing_target_window"
        }
    );
    assert_eq!(adapter.restore_calls, 0);
    assert_eq!(adapter.paste_calls, 0);
}

#[cfg(windows)]
#[test]
fn failed_restore_falls_back_without_sending_input() {
    let mut adapter = FakeAdapter {
        restore_fails: true,
        ..Default::default()
    };
    let result =
        inject_with_adapter("hello", InjectMode::AutoPaste, Some(7), &mut adapter).unwrap();

    assert_eq!(
        result.delivery,
        Delivery::ClipboardFallback {
            reason: "foreground_restore_failed"
        }
    );
    assert_eq!(adapter.restore_calls, 1);
    assert_eq!(adapter.paste_calls, 0);
}

#[cfg(windows)]
#[test]
fn elevated_target_mismatch_falls_back_without_sending_input() {
    // SetForegroundWindow is expected to fail when the target is elevated
    // beyond the shell process; treat that the same as any unsafe restore.
    let mut adapter = FakeAdapter {
        restore_fails: true,
        ..Default::default()
    };
    let result =
        inject_with_adapter("hello", InjectMode::AutoPaste, Some(7), &mut adapter).unwrap();

    assert_eq!(
        result.delivery,
        Delivery::ClipboardFallback {
            reason: "foreground_restore_failed"
        }
    );
    assert_eq!(adapter.paste_calls, 0);
}

#[cfg(windows)]
#[test]
fn stale_target_falls_back_without_sending_input() {
    let mut adapter = FakeAdapter {
        restore_fails: true,
        restore_stale: true,
        ..Default::default()
    };
    let result =
        inject_with_adapter("hello", InjectMode::AutoPaste, Some(7), &mut adapter).unwrap();

    assert_eq!(
        result.delivery,
        Delivery::ClipboardFallback {
            reason: "stale_target_window"
        }
    );
    assert_eq!(adapter.paste_calls, 0);
}

#[cfg(windows)]
#[test]
fn target_switch_falls_back_without_sending_input() {
    let mut adapter = FakeAdapter {
        foreground: Some(8),
        ..Default::default()
    };
    let result =
        inject_with_adapter("hello", InjectMode::AutoPaste, Some(7), &mut adapter).unwrap();

    assert_eq!(
        result.delivery,
        Delivery::ClipboardFallback {
            reason: "target_window_changed"
        }
    );
    assert_eq!(adapter.paste_calls, 0);
}

#[cfg(target_os = "macos")]
#[test]
fn failed_paste_falls_back_to_clipboard() {
    // The macOS inject path has no foreground restore: a failed Cmd+V is
    // the only fallback, and the text stays on the clipboard.
    let mut adapter = FakeAdapter {
        paste_fails: true,
        ..Default::default()
    };
    let result =
        inject_with_adapter("hello", InjectMode::AutoPaste, Some(7), &mut adapter).unwrap();

    assert_eq!(
        result.delivery,
        Delivery::ClipboardFallback {
            reason: "paste_failed"
        }
    );
    assert_eq!(adapter.clipboard_calls, 1);
    assert_eq!(adapter.paste_calls, 1);
}

#[cfg(windows)]
#[test]
fn windows_terminal_uses_terminal_paste_shortcut() {
    // Regression: 2026-07-03. Windows Terminal does not reliably paste on plain Ctrl+V.
    assert_eq!(
        paste_shortcut_for_window_class("CASCADIA_HOSTING_WINDOW_CLASS"),
        PasteShortcut::ShiftInsert
    );
    assert_eq!(
        paste_shortcut_for_window_class("Chrome_WidgetWin_1"),
        PasteShortcut::CtrlV
    );
}

#[cfg(windows)]
#[test]
#[ignore = "requires an interactive Windows session with a focused editable control"]
fn windows_real_focus_paste_smoke() {
    assert_eq!(
        std::env::var("MUNDUS_DICTATION_WINDOWS_SMOKE").as_deref(),
        Ok("1"),
        "set MUNDUS_DICTATION_WINDOWS_SMOKE=1 to run the real paste smoke test"
    );
    let target = capture_foreground_window().expect("an interactive foreground window");
    let mut adapter = SystemOsAdapter;
    let result = inject_with_adapter(
        "Mundus dictation Windows smoke",
        InjectMode::AutoPaste,
        Some(target),
        &mut adapter,
    )
    .expect("clipboard and paste delivery should succeed");
    assert_eq!(result.delivery, Delivery::Pasted);
}
