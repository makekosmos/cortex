import Foundation
import CoreGraphics
import Darwin

final class MonitorState {
    let targetKeyCode: CGKeyCode
    let needCmd: Bool
    let needCtrl: Bool
    let needAlt: Bool
    let needShift: Bool
    let needFn: Bool
    var isPressed: Bool

    init(targetKeyCode: CGKeyCode, needCmd: Bool, needCtrl: Bool, needAlt: Bool, needShift: Bool, needFn: Bool) {
        self.targetKeyCode = targetKeyCode
        self.needCmd = needCmd
        self.needCtrl = needCtrl
        self.needAlt = needAlt
        self.needShift = needShift
        self.needFn = needFn
        self.isPressed = false
    }
}

func emit(_ payload: [String: Any]) {
    guard
        let data = try? JSONSerialization.data(withJSONObject: payload, options: []),
        let text = String(data: data, encoding: .utf8)
    else { return }
    print(text)
    fflush(stdout)
}

func parseBool(_ raw: String?) -> Bool {
    guard let raw else { return false }
    return raw == "1" || raw.lowercased() == "true"
}

func installParentWatchdogIfNeeded() {
    guard
        let raw = ProcessInfo.processInfo.environment["MUNDUS_PARENT_PID"],
        let parentPid = Int32(raw)
    else { return }

    let timer = CFRunLoopTimerCreateWithHandler(kCFAllocatorDefault, CFAbsoluteTimeGetCurrent() + 1.0, 1.0, 0, 0) { _ in
        if kill(parentPid, 0) != 0 {
            emit(["released": true, "reason": "parent-exit"])
            exit(0)
        }
    }
    CFRunLoopAddTimer(CFRunLoopGetCurrent(), timer, .commonModes)
}

func modifiersSatisfied(flags: CGEventFlags, state: MonitorState) -> Bool {
    let cmd = flags.contains(.maskCommand)
    let ctrl = flags.contains(.maskControl)
    let alt = flags.contains(.maskAlternate)
    let shift = flags.contains(.maskShift)
    let fn = flags.contains(.maskSecondaryFn)
    if cmd != state.needCmd { return false }
    if ctrl != state.needCtrl { return false }
    if alt != state.needAlt { return false }
    if shift != state.needShift { return false }
    if fn != state.needFn { return false }
    return true
}

guard CommandLine.arguments.count >= 7 else {
    emit(["error": "Usage: hotkey-hold-monitor <keyCode> <cmd0|1> <ctrl0|1> <alt0|1> <shift0|1> <fn0|1> [doubleFn0|1]"])
    exit(1)
}

guard let keyCodeRaw = Int(CommandLine.arguments[1]), keyCodeRaw >= 0 else {
    emit(["error": "Invalid keyCode"])
    exit(1)
}

let state = MonitorState(
    targetKeyCode: CGKeyCode(keyCodeRaw),
    needCmd: parseBool(CommandLine.arguments[2]),
    needCtrl: parseBool(CommandLine.arguments[3]),
    needAlt: parseBool(CommandLine.arguments[4]),
    needShift: parseBool(CommandLine.arguments[5]),
    needFn: parseBool(CommandLine.arguments[6])
)

// Args 8+: doubleFn — хоткей "FnFn" (дабл-тап Fn, как системная диктовка).
let wantDoubleFn = CommandLine.arguments.count >= 8 && parseBool(CommandLine.arguments[7])

// Дабл-Esc (keyCode 53) — отмена записи на всех платформах; наблюдаем,
// событие не перехватываем.
let kEscKeyCode: CGKeyCode = 53
let kFnKeyCode: CGKeyCode = 63
let kDoubleTapMs: Double = 0.5
var lastEscDown = Date.distantPast
var lastFnDown = Date.distantPast

let statePtr = Unmanaged.passRetained(state).toOpaque()
let eventMask: CGEventMask =
    (1 << CGEventType.keyDown.rawValue) |
    (1 << CGEventType.keyUp.rawValue) |
    (1 << CGEventType.flagsChanged.rawValue)

let callback: CGEventTapCallBack = { _, type, event, userInfo in
    guard let userInfo else { return Unmanaged.passUnretained(event) }
    let state = Unmanaged<MonitorState>.fromOpaque(userInfo).takeUnretainedValue()

    if type == .tapDisabledByTimeout || type == .tapDisabledByUserInput {
        return Unmanaged.passUnretained(event)
    }

    let flags = event.flags
    let keyCode = CGKeyCode(event.getIntegerValueField(.keyboardEventKeycode))
    let now = Date()

    // Дабл-Esc — отмена (emit побочно, основной матч ниже не затрагивается).
    if type == .keyDown && keyCode == kEscKeyCode {
        if now.timeIntervalSince(lastEscDown) < kDoubleTapMs {
            emit(["escapeCancel": true])
        }
        lastEscDown = now
    }

    // Дабл-тап Fn — срабатывание хоткея "FnFn" и выход (respawn со стороны Rust).
    if wantDoubleFn && type == .flagsChanged && keyCode == kFnKeyCode {
        if flags.contains(.maskSecondaryFn) {
            if now.timeIntervalSince(lastFnDown) < kDoubleTapMs {
                emit(["pressed": true])
                exit(0)
            }
            lastFnDown = now
        }
        return Unmanaged.passUnretained(event)
    }

    if !state.isPressed {
        if (type == .keyDown || type == .flagsChanged) && keyCode == state.targetKeyCode && modifiersSatisfied(flags: flags, state: state) {
            state.isPressed = true
            emit(["pressed": true])
        }
        return Unmanaged.passUnretained(event)
    }

    if type == .keyUp {
        if keyCode == state.targetKeyCode {
            emit(["released": true, "reason": "key-up"])
            exit(0)
        }
        // If any required modifier is no longer active, treat as release.
        if !modifiersSatisfied(flags: flags, state: state) {
            emit(["released": true, "reason": "modifier-up"])
            exit(0)
        }
    }

    if type == .flagsChanged {
        if !modifiersSatisfied(flags: flags, state: state) {
            emit(["released": true, "reason": "flags-changed"])
            exit(0)
        }
    }

    return Unmanaged.passUnretained(event)
}

guard let eventTap = CGEvent.tapCreate(
    tap: .cghidEventTap,
    place: .headInsertEventTap,
    options: .listenOnly,
    eventsOfInterest: eventMask,
    callback: callback,
    userInfo: statePtr
) else {
    emit([
        "error": "Failed to create event tap. Enable Input Monitoring/Accessibility permissions for SuperCmd."
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
