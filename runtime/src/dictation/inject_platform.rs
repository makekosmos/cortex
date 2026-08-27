use super::{Delivery, DeliveryResult, InjectError, InjectMode, OsAdapter};

#[cfg(windows)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PasteShortcut {
    CtrlV,
    ShiftInsert,
}

#[cfg(windows)]
pub(crate) fn paste_shortcut_for_window_class(class_name: &str) -> PasteShortcut {
    let class = class_name.to_ascii_lowercase();
    let terminal_classes = [
        "cascadia_hosting_window_class",
        "consolewindowclass",
        "mintty",
        "wezterm",
        "alacritty",
        "virtualconsoleclass",
    ];
    if terminal_classes.iter().any(|needle| class.contains(needle)) {
        PasteShortcut::ShiftInsert
    } else {
        PasteShortcut::CtrlV
    }
}

#[cfg(windows)]
pub fn capture_foreground_window() -> Option<isize> {
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    let hwnd = unsafe { GetForegroundWindow() };
    let raw = hwnd.0 as isize;
    (raw != 0).then_some(raw)
}

#[cfg(not(windows))]
pub fn capture_foreground_window() -> Option<isize> {
    None
}

#[cfg(windows)]
fn window_class_name(raw: isize) -> Option<String> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::GetClassNameW;

    let hwnd = HWND(raw as *mut _);
    let mut buf = [0u16; 256];
    let len = unsafe { GetClassNameW(hwnd, &mut buf) };
    (len > 0).then(|| String::from_utf16_lossy(&buf[..len as usize]))
}

#[cfg(windows)]
fn send_paste_shortcut(shortcut: PasteShortcut) -> Result<(), InjectError> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
        VIRTUAL_KEY, VK_CONTROL, VK_INSERT, VK_SHIFT,
    };

    const VK_V: u16 = 0x56;

    fn make_key(vk: u16, key_up: bool) -> INPUT {
        let flags = if key_up {
            KEYEVENTF_KEYUP
        } else {
            KEYBD_EVENT_FLAGS(0)
        };
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(vk),
                    wScan: 0,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    let (modifier, key) = match shortcut {
        PasteShortcut::CtrlV => (VK_CONTROL.0, VK_V),
        PasteShortcut::ShiftInsert => (VK_SHIFT.0, VK_INSERT.0),
    };
    let inputs = [
        make_key(modifier, false),
        make_key(key, false),
        make_key(key, true),
        make_key(modifier, true),
    ];
    let cb = std::mem::size_of::<INPUT>() as i32;
    let injected = unsafe { SendInput(&inputs, cb) };
    if injected as usize != inputs.len() {
        return Err(InjectError::SendInput {
            injected,
            expected: inputs.len() as u32,
        });
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn send_macos_paste() -> Result<(), InjectError> {
    let script = r#"tell application "System Events" to keystroke "v" using command down"#;
    let status = std::process::Command::new("/usr/bin/osascript")
        .args(["-e", script])
        .status()
        .map_err(|_| InjectError::SendInput {
            injected: 0,
            expected: 1,
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(InjectError::SendInput {
            injected: 0,
            expected: 1,
        })
    }
}

pub(crate) struct SystemOsAdapter;

impl OsAdapter for SystemOsAdapter {
    fn set_clipboard(&mut self, text: &str) -> Result<(), InjectError> {
        let mut clipboard = arboard::Clipboard::new()?;
        clipboard.set_text(text.to_owned())?;
        Ok(())
    }

    fn foreground_window(&mut self) -> Option<isize> {
        capture_foreground_window()
    }

    fn restore_foreground_window(&mut self, raw: isize) -> Result<(), InjectError> {
        #[cfg(windows)]
        {
            use windows::Win32::Foundation::HWND;
            use windows::Win32::UI::WindowsAndMessaging::{
                GetForegroundWindow, IsWindow, SetForegroundWindow,
            };

            let hwnd = HWND(raw as *mut _);
            if !unsafe { IsWindow(hwnd).as_bool() } {
                return Err(InjectError::StaleTarget);
            }
            if unsafe { GetForegroundWindow() } != hwnd
                && !unsafe { SetForegroundWindow(hwnd).as_bool() }
            {
                return Err(InjectError::ForegroundRestore);
            }
            if unsafe { GetForegroundWindow() } != hwnd {
                return Err(InjectError::ForegroundRestore);
            }
        }
        #[cfg(not(windows))]
        {
            let _ = raw;
        }
        Ok(())
    }

    #[cfg(windows)]
    fn window_class_name(&mut self, raw: isize) -> Option<String> {
        window_class_name(raw)
    }

    #[cfg(windows)]
    fn send_paste(&mut self, shortcut: PasteShortcut) -> Result<(), InjectError> {
        send_paste_shortcut(shortcut)
    }

    #[cfg(target_os = "macos")]
    fn send_paste(&mut self) -> Result<(), InjectError> {
        send_macos_paste()
    }
}

pub(crate) fn inject_with_adapter(
    text: &str,
    mode: InjectMode,
    prev_hwnd: Option<isize>,
    adapter: &mut dyn OsAdapter,
) -> Result<DeliveryResult, InjectError> {
    adapter.set_clipboard(text)?;
    if matches!(mode, InjectMode::ClipboardOnly) {
        return Ok(DeliveryResult {
            delivery: Delivery::ClipboardOnly,
        });
    }

    #[cfg(target_os = "macos")]
    {
        let _ = prev_hwnd;
        if let Err(error) = adapter.send_paste() {
            return Ok(DeliveryResult {
                delivery: Delivery::ClipboardFallback {
                    reason: error.safe_reason(),
                },
            });
        }
        std::thread::sleep(std::time::Duration::from_millis(80));
        return Ok(DeliveryResult {
            delivery: Delivery::Pasted,
        });
    }

    #[cfg(not(target_os = "macos"))]
    {
        let Some(hwnd) = prev_hwnd else {
            return Ok(DeliveryResult {
                delivery: Delivery::ClipboardFallback {
                    reason: InjectError::MissingTarget.safe_reason(),
                },
            });
        };
        if let Err(error) = adapter.restore_foreground_window(hwnd) {
            return Ok(DeliveryResult {
                delivery: Delivery::ClipboardFallback {
                    reason: error.safe_reason(),
                },
            });
        }

        std::thread::sleep(std::time::Duration::from_millis(80));
        if adapter.foreground_window() != Some(hwnd) {
            return Ok(DeliveryResult {
                delivery: Delivery::ClipboardFallback {
                    reason: InjectError::TargetChanged.safe_reason(),
                },
            });
        }

        #[cfg(windows)]
        let shortcut = adapter
            .window_class_name(hwnd)
            .map(|class| paste_shortcut_for_window_class(&class))
            .unwrap_or(PasteShortcut::CtrlV);
        if adapter.foreground_window() != Some(hwnd) {
            return Ok(DeliveryResult {
                delivery: Delivery::ClipboardFallback {
                    reason: InjectError::TargetChanged.safe_reason(),
                },
            });
        }

        #[cfg(windows)]
        if let Err(error) = adapter.send_paste(shortcut) {
            return Ok(DeliveryResult {
                delivery: Delivery::ClipboardFallback {
                    reason: error.safe_reason(),
                },
            });
        }

        #[cfg(not(windows))]
        return Ok(DeliveryResult {
            delivery: Delivery::ClipboardFallback {
                reason: "paste_unavailable",
            },
        });

        #[cfg(windows)]
        {
            std::thread::sleep(std::time::Duration::from_millis(80));
            Ok(DeliveryResult {
                delivery: Delivery::Pasted,
            })
        }
    }
}
