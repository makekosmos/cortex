import Foundation
import AppKit
import CoreGraphics
import Darwin

// KOS-376: bundled helper — without the accessory policy its event tap would
// claim a second Dock tile under the Mundus Manager identity.
NSApplication.shared.setActivationPolicy(.accessory)

// capture-hotkey — variation point hotkey-hold-monitor для назначения хоткея.
//
// В отличие от hold-monitor (следит за КОНКРЕТНЫМ keyCode) — здесь нет фильтра
// по клавише: ловим первый keyDown с ≥1 квалифицирующим модификатором
// (cmd/ctrl/alt/shift) и эмитим сочетание, затем выходим. Escape (keyCode 53) —
// отмена. Rust-сторона (macos_native::begin_capture) конвертирует keyCode →
// accelerator-строку и эмитит в общий dictation broadcast-канал.

func emit(_ payload: [String: Any]) {
    guard
        let data = try? JSONSerialization.data(withJSONObject: payload, options: []),
        let text = String(data: data, encoding: .utf8)
    else { return }
    print(text)
    fflush(stdout)
}

func installParentWatchdogIfNeeded() {
    guard
        let raw = ProcessInfo.processInfo.environment["MUNDUS_PARENT_PID"],
        let parentPid = Int32(raw)
    else { return }

    let timer = CFRunLoopTimerCreateWithHandler(kCFAllocatorDefault, CFAbsoluteTimeGetCurrent() + 1.0, 1.0, 0, 0) { _ in
        if kill(parentPid, 0) != 0 {
            emit(["cancelled": true, "reason": "parent-exit"])
            exit(0)
        }
    }
    CFRunLoopAddTimer(CFRunLoopGetCurrent(), timer, .commonModes)
}

let kEscapeKeyCode: CGKeyCode = 53
let kFnKeyCode: CGKeyCode = 63
let kDoubleTapMs: Double = 0.5
var lastFnDown = Date.distantPast

let eventMask: CGEventMask =
    (1 << CGEventType.keyDown.rawValue) |
    (1 << CGEventType.flagsChanged.rawValue)

let callback: CGEventTapCallBack = { _, type, event, _ in
    if type == .tapDisabledByTimeout || type == .tapDisabledByUserInput {
        return Unmanaged.passUnretained(event)
    }

    let keyCode = CGKeyCode(event.getIntegerValueField(.keyboardEventKeycode))

    // Дабл-тап Fn — отдельный хоткей "FnFn" (Fn одиночной не биндится).
    if type == .flagsChanged && keyCode == kFnKeyCode {
        if event.flags.contains(.maskSecondaryFn) {
            if Date().timeIntervalSince(lastFnDown) < kDoubleTapMs {
                emit(["captured": true, "doubleFn": true])
                exit(0)
            }
            lastFnDown = Date()
        }
        return Unmanaged.passUnretained(event)
    }
    if type != .keyDown {
        return Unmanaged.passUnretained(event)
    }

    // Escape — отмена capture (с модификаторами или без).
    if keyCode == kEscapeKeyCode {
        emit(["cancelled": true, "reason": "escape"])
        exit(0)
    }

    let flags = event.flags
    let cmd = flags.contains(.maskCommand)
    let ctrl = flags.contains(.maskControl)
    let alt = flags.contains(.maskAlternate)
    let shift = flags.contains(.maskShift)
    let fn = flags.contains(.maskSecondaryFn)

    // Требуем ≥1 квалифицирующий модификатор (как Windows-ветка: одиночная
    // клавиша = обычный input, не shortcut). fn в зачёт не идёт — он редко
    // используется как самостоятельный chord-модификатор.
    if !(cmd || ctrl || alt || shift) {
        return Unmanaged.passUnretained(event)
    }

    emit([
        "captured": true,
        "keyCode": Int(keyCode),
        "cmd": cmd,
        "ctrl": ctrl,
        "alt": alt,
        "shift": shift,
        "fn": fn,
    ])
    exit(0)
}

guard let eventTap = CGEvent.tapCreate(
    tap: .cghidEventTap,
    place: .headInsertEventTap,
    options: .listenOnly,
    eventsOfInterest: eventMask,
    callback: callback,
    userInfo: nil
) else {
    emit([
        "error": "Failed to create event tap. Enable Input Monitoring/Accessibility permissions."
    ])
    exit(2)
}

guard let source = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, eventTap, 0) else {
    emit(["error": "Failed to create run loop source"])
    exit(2)
}

CFRunLoopAddSource(CFRunLoopGetCurrent(), source, .commonModes)
installParentWatchdogIfNeeded()
CGEvent.tapEnable(tap: eventTap, enable: true)
emit(["ready": true])
CFRunLoopRun()
