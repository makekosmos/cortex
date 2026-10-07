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

/// macOS: frontmost app PID without any TCC permission — `lsappinfo front`
/// returns the frontmost app's ASN, `lsappinfo info -only pid <ASN>` reports
/// its pid. Unlike the System Events osascript path this needs no Automation
/// grant and does not stall on a permission prompt.
#[cfg(target_os = "macos")]
fn lsappinfo_frontmost_pid() -> Option<isize> {
    let out = std::process::Command::new("/usr/bin/lsappinfo")
        .arg("front")
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let asn = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    if asn.is_empty() {
        return None;
    }
    let out = std::process::Command::new("/usr/bin/lsappinfo")
        .args(["info", "-only", "pid", &asn])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    // Output looks like: `pid = 74310 !cgsConnection ...`
    let text = String::from_utf8_lossy(&out.stdout);
    let after_pid = &text[text.find("pid")? + "pid".len()..];
    let after_eq = &after_pid[after_pid.find('=')? + 1..];
    let digits: String = after_eq
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse::<isize>().ok()
}

/// macOS: frontmost app PID — stored as prev_hwnd and used to reactivate
/// the target before Cmd+V at delivery time. lsappinfo first (no TCC
/// permission), osascript System Events as fallback.
#[cfg(target_os = "macos")]
pub fn capture_foreground_window() -> Option<isize> {
    if let Some(pid) = lsappinfo_frontmost_pid() {
        eprintln!("[dictation::inject] capture_foreground_window via lsappinfo: {pid}");
        return Some(pid);
    }
    eprintln!("[dictation::inject] lsappinfo capture failed, falling back to osascript");
    const SCRIPT: &str = concat!(
        r#"tell application "System Events" to "#,
        "get unix id of first application process whose frontmost is true",
    );
    let out = std::process::Command::new("/usr/bin/osascript")
        .args(["-e", SCRIPT])
        .output()
        .ok()?;
    if !out.status.success() {
        eprintln!(
            "[dictation::inject] osascript capture failed: status {:?}",
            out.status
        );
        return None;
    }
    let pid = String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse::<isize>()
        .ok();
    eprintln!("[dictation::inject] capture_foreground_window via osascript: {pid:?}");
    pid
}

#[cfg(all(not(windows), not(target_os = "macos")))]
pub fn capture_foreground_window() -> Option<isize> {
    None
}

/// Reactivate the app captured at recording start so the paste keystroke
/// lands in the user's editor, not in the dictation window itself.
#[cfg(target_os = "macos")]
fn activate_pid(pid: isize) -> Result<(), InjectError> {
    let script = format!(
        "tell application \"System Events\" to set frontmost of \
         (first process whose unix id is {pid}) to true"
    );
    let ok = std::process::Command::new("/usr/bin/osascript")
        .args(["-e", &script])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if ok {
        Ok(())
    } else {
        Err(InjectError::ForegroundRestore)
    }
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

/// Result of the `paste-text` helper: it activates the captured pid, waits
/// for real frontmost, and posts Cmd+V through the HID event tap — pasting
/// works in any paste-capable control, not only AX-recognised text fields.
#[cfg(target_os = "macos")]
enum NativePaste {
    Unavailable,
    Failed(&'static str),
}

#[cfg(all(target_os = "macos", test))]
fn paste_via_native(_pid: isize) -> Result<(), NativePaste> {
    Err(NativePaste::Unavailable)
}

#[cfg(all(target_os = "macos", not(test)))]
fn paste_via_native(pid: isize) -> Result<(), NativePaste> {
    let helper = match crate::macos_native::resolve_helper_pub("paste-text") {
        Ok(helper) => {
            eprintln!(
                "[dictation::inject] paste-text helper resolved: {}",
                helper.display()
            );
            helper
        }
        Err(error) => {
            eprintln!("[dictation::inject] paste-text helper NOT resolved: {error}");
            return Err(NativePaste::Unavailable);
        }
    };
    let out = match std::process::Command::new(&helper)
        .arg(pid.to_string())
        .output()
    {
        Ok(out) => out,
        Err(error) => {
            eprintln!("[dictation::inject] paste-text spawn failed: {error}");
            return Err(NativePaste::Failed("helper_spawn_failed"));
        }
    };
    let stdout = String::from_utf8_lossy(&out.stdout);
    eprintln!(
        "[dictation::inject] paste-text exit={:?} stdout={}",
        out.status,
        stdout.trim()
    );
    let value: serde_json::Value = serde_json::from_str(stdout.trim())
        .map_err(|_| NativePaste::Failed("helper_bad_output"))?;
    if value.get("ok").and_then(|v| v.as_bool()) == Some(true) {
        return Ok(());
    }
    let reason = match value.get("reason").and_then(|v| v.as_str()) {
        Some("accessibility_denied") => "accessibility_denied",
        Some("activation_failed") => "foreground_restore_failed",
        Some("no_app") | Some("bad_args") => "stale_target_window",
        _ => "paste_failed",
    };
    Err(NativePaste::Failed(reason))
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
            if !unsafe { IsWindow(Some(hwnd)).as_bool() } {
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
        #[cfg(target_os = "macos")]
        {
            activate_pid(raw)?;
        }
        #[cfg(all(not(windows), not(target_os = "macos")))]
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
        eprintln!("[dictation::inject] inject_with_adapter prev_hwnd={prev_hwnd:?}");
        if let Some(pid) = prev_hwnd {
            match paste_via_native(pid) {
                Ok(()) => {
                    std::thread::sleep(std::time::Duration::from_millis(80));
                    eprintln!("[dictation::inject] delivery=pasted via paste-text pid={pid}");
                    return Ok(DeliveryResult {
                        delivery: Delivery::Pasted,
                    });
                }
                Err(NativePaste::Unavailable) => {
                    eprintln!("[dictation::inject] paste-text unavailable — osascript fallback");
                    // Helper not built — legacy osascript path.
                    if let Err(error) = adapter.restore_foreground_window(pid) {
                        return Ok(DeliveryResult {
                            delivery: Delivery::ClipboardFallback {
                                reason: error.safe_reason(),
                            },
                        });
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                Err(NativePaste::Failed(reason)) => {
                    eprintln!("[dictation::inject] delivery=clipboard_fallback reason={reason}");
                    return Ok(DeliveryResult {
                        delivery: Delivery::ClipboardFallback { reason },
                    });
                }
            }
        }
        if let Err(error) = adapter.send_paste() {
            eprintln!(
                "[dictation::inject] delivery=clipboard_fallback reason={}",
                error.safe_reason()
            );
            return Ok(DeliveryResult {
                delivery: Delivery::ClipboardFallback {
                    reason: error.safe_reason(),
                },
            });
        }
        std::thread::sleep(std::time::Duration::from_millis(80));
        eprintln!("[dictation::inject] delivery=pasted via osascript keystroke");
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
