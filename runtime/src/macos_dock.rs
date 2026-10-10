// KOS-376: `mundus-engine` ships inside `Mundus Manager.app/Contents/MacOS`,
// so on macOS it inherits the bundle's identity and the default `Regular`
// activation policy. Its first WindowServer connection (arboard's
// NSPasteboard during dictation inject, spawned AppKit-linked helpers, AX
// calls) would then register it as a foreground app and the Dock renders a
// second "Mundus Manager" tile next to the real one.
//
// Transforming the process to UIElement — the runtime equivalent of
// `LSUIElement` — keeps it a background agent: no Dock tile and no Cmd-Tab
// entry, while clipboard, event taps and TCC prompts keep working. It is a
// no-op on other platforms (the Engine's Windows tray is unaffected: it has
// no Dock).

/// Convert this process from `Regular` to `UIElement` activation policy.
/// Safe to call before any AppKit use and before the runtime starts.
pub fn suppress_dock_tile() {
    suppress_dock_tile_impl();
}

#[cfg(target_os = "macos")]
fn suppress_dock_tile_impl() {
    // HIServices C API — reachable without initializing NSApplication, so the
    // policy is already UIElement when the first WindowServer connection
    // happens later.
    const K_PROCESS_TRANSFORM_TO_UI_ELEMENT_APPLICATION: u32 = 4;

    #[repr(C)]
    struct ProcessSerialNumber {
        high_long_of_psn: u32,
        low_long_of_psn: u32,
    }

    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn GetCurrentProcess(psn: *mut ProcessSerialNumber) -> i16;
        fn TransformProcessType(psn: *const ProcessSerialNumber, state: u32) -> i32;
    }

    let mut psn = ProcessSerialNumber {
        high_long_of_psn: 0,
        low_long_of_psn: 0,
    };
    // A failed transform only means the process keeps its default policy;
    // nothing the Engine does depends on the Dock either way, so this is
    // best-effort rather than fatal.
    if unsafe { GetCurrentProcess(&mut psn) } != 0 {
        return;
    }
    unsafe {
        TransformProcessType(&psn, K_PROCESS_TRANSFORM_TO_UI_ELEMENT_APPLICATION);
    }
}

#[cfg(not(target_os = "macos"))]
fn suppress_dock_tile_impl() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suppress_dock_tile_is_safe_off_macos() {
        // On Linux CI this exercises the no-op path; on macOS CI it must be
        // idempotent (the test runner process is already a UI element under
        // cargo nextest).
        suppress_dock_tile();
        suppress_dock_tile();
    }
}
