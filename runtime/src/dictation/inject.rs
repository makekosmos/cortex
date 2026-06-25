// Inject — пишет transcript в clipboard, опционально симулирует Ctrl+V
// в предыдущем активном окне (HWND, захваченный до показа pill).
//
// Поток AutoPaste (см. forbidden.md → Dictation, обязательные задержки):
//   1. сохранить оригинальный clipboard text
//   2. clipboard.set_text(transcript)
//   3. SetForegroundWindow(prev_hwnd) если есть HWND
//   4. sleep 80ms — даём ОС вернуть focus в целевое окно
//   5. enigo: Ctrl press → V click → Ctrl release
//   6. sleep 80ms — даём ОС прочитать буфер ПЕРЕД restore (иначе race:
//      Windows может ещё не успеть paste'нуть → восстановим старый текст
//      раньше времени → вставится старый)
//   7. clipboard.set_text(original) — restore
//
// Поток ClipboardOnly: только шаг 2. Юзер сам жмёт Ctrl+V.
//
// Все API calls в enigo/arboard синхронные + потенциально блокирующие
// (особенно SendInput на медленных машинах). Вызываем через
// `tokio::task::spawn_blocking` в host.

use std::thread;
use std::time::Duration;

use thiserror::Error;

use super::config::InjectMode;

/// Минимальная задержка после Ctrl+V до restore'а clipboard. 80ms даёт запас
/// над типичными ~30-50ms которые Windows тратит на обработку paste
/// (особенно медленный когда target — Electron-based app типа VS Code).
const POST_PASTE_DELAY_MS: u64 = 80;

/// Задержка после SetForegroundWindow перед симуляцией клавиш. Без неё
/// `Ctrl+V` может уйти в pill window (фокус ещё не вернулся).
const REFOCUS_DELAY_MS: u64 = 80;

#[derive(Debug, Error)]
pub enum InjectError {
    #[error("clipboard: {0}")]
    Clipboard(#[from] arboard::Error),
    #[error("SendInput failed: injected {injected} of {expected} events")]
    SendInput { injected: u32, expected: u32 },
    #[error("target window changed before paste")]
    TargetWindowChanged,
}

/// Захват активного окна на момент вызова. Должен вызываться ДО показа
/// pill (иначе foreground = pill). Возвращаемое значение — opaque HWND
/// для передачи в `inject`. None на не-Windows / при отсутствии foreground.
#[cfg(windows)]
pub fn capture_foreground_window() -> Option<isize> {
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    let hwnd = unsafe { GetForegroundWindow() };
    let raw = hwnd.0 as isize;
    if raw == 0 {
        None
    } else {
        Some(raw)
    }
}

#[cfg(not(windows))]
pub fn capture_foreground_window() -> Option<isize> {
    None
}

#[cfg(windows)]
fn restore_foreground_window(raw: isize) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow;
    let hwnd = HWND(raw as *mut _);
    unsafe {
        let _ = SetForegroundWindow(hwnd);
    }
}

#[cfg(not(windows))]
fn restore_foreground_window(_raw: isize) {}

/// Симулирует Ctrl+V через Win32 SendInput. Используем VK_CONTROL + VK_V
/// (0x56) — стандартный virtual-key канал. enigo путь через
/// `Key::Unicode('v')` шлёт VK_PACKET (Unicode channel), на котором
/// модификаторы (Ctrl) не работают как shortcut и сам enigo падал
/// с `TryFromIntError` при попытке упаковать keystate в u32.
#[cfg(windows)]
fn send_ctrl_v() -> Result<(), InjectError> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
        VIRTUAL_KEY, VK_CONTROL,
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

    let inputs = [
        make_key(VK_CONTROL.0, false),
        make_key(VK_V, false),
        make_key(VK_V, true),
        make_key(VK_CONTROL.0, true),
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

#[cfg(all(not(windows), not(target_os = "macos")))]
fn send_ctrl_v() -> Result<(), InjectError> {
    // Phase 1 Windows-only. На non-Windows автоинжект не реализован — пользователь
    // получит транскрипт в clipboard и Ctrl+V руками.
    Ok(())
}

#[cfg(target_os = "macos")]
fn send_ctrl_v() -> Result<(), InjectError> {
    let script = r#"tell application "System Events" to keystroke "v" using command down"#;
    let status = std::process::Command::new("/usr/bin/osascript")
        .args(["-e", script])
        .status()
        .map_err(|_| InjectError::SendInput {
            injected: 0,
            expected: 1,
        })?;
    if !status.success() {
        return Err(InjectError::SendInput {
            injected: 0,
            expected: 1,
        });
    }
    Ok(())
}

/// Blocking impl. Вызывать из `spawn_blocking`. Восстановление clipboard
/// best-effort: если оригинала не было (текст недоступен / image / files
/// — пока не поддерживаем) — оставляем transcript в буфере.
pub fn inject_blocking(
    text: &str,
    mode: InjectMode,
    prev_hwnd: Option<isize>,
) -> Result<(), InjectError> {
    let mut clipboard = arboard::Clipboard::new()?;
    let original = clipboard.get_text().ok();

    clipboard.set_text(text.to_owned())?;

    if matches!(mode, InjectMode::ClipboardOnly) {
        return Ok(());
    }

    // Если юзер ушёл из target окна (current foreground != prev_hwnd) —
    // НЕ воруем фокус. Текст остаётся в clipboard, юзер сам нажмёт Ctrl+V
    // когда будет готов. Это критично для auto-retry в фоне: если ретрай
    // завершается пока юзер уже работает в другом приложении, мы не дёргаем
    // его фокус и не вставляем текст в неправильное окно.
    if let Some(prev) = prev_hwnd {
        let current = capture_foreground_window();
        if current != Some(prev) {
            // Foreground сменился — transcript уже в clipboard, но auto-paste не
            // случился. Пусть UI явно скажет об этом вместо тихого success.
            return Err(InjectError::TargetWindowChanged);
        }
    }

    if let Some(hwnd) = prev_hwnd {
        restore_foreground_window(hwnd);
    }
    thread::sleep(Duration::from_millis(REFOCUS_DELAY_MS));

    send_ctrl_v()?;

    thread::sleep(Duration::from_millis(POST_PASTE_DELAY_MS));

    if let Some(orig) = original {
        // best-effort restore — если не смогли, пользователь увидит transcript
        // в буфере. Не считаем за ошибку всего inject'а.
        let _ = clipboard.set_text(orig);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_returns_some_or_none_without_panic() {
        // Не assert'им конкретное значение — зависит от среды (CI без UI = None).
        let _ = capture_foreground_window();
    }

    // Реальный inject_blocking требует interactive UI session + active window
    // — не запускаем в `cargo test` (CI не имеет foreground). Покрыто manual
    // visual verify per spec AC5.
}
