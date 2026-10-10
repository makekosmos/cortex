import Foundation
import AppKit
import ApplicationServices
import CoreGraphics

// KOS-376: bundled helper — NSWorkspace/activate() connects it to the
// WindowServer; without the accessory policy it would claim a second Dock
// tile under the Mundus Manager identity.
NSApplication.shared.setActivationPolicy(.accessory)

private let debugEnabled = ProcessInfo.processInfo.environment["PASTE_TEXT_DEBUG"] == "1"

private func dbg(_ message: @autoclosure () -> String) {
  if debugEnabled {
    FileHandle.standardError.write(Data(("[paste-text] " + message() + "\n").utf8))
  }
}

private func emit(_ ok: Bool, _ reason: String? = nil) -> Never {
  var dict: [String: Any] = ["ok": ok]
  if let reason { dict["reason"] = reason }
  if let data = try? JSONSerialization.data(withJSONObject: dict),
     let line = String(data: data, encoding: .utf8) {
    FileHandle.standardOutput.write(Data((line + "\n").utf8))
  }
  exit(ok ? 0 : 1)
}

// Usage: paste-text <pid>
// Clipboard is already populated by the Rust side; this helper activates the
// captured app, waits until it is truly frontmost, then posts a real Cmd+V
// through the HID event tap — works in any paste-capable control, including
// places System Events does not recognise as a text field.
guard CommandLine.arguments.count > 1,
      let rawPid = Int32(CommandLine.arguments[1]) else {
  emit(false, "bad_args")
}
let targetPid = pid_t(rawPid)
guard let app = NSRunningApplication(processIdentifier: targetPid) else {
  emit(false, "no_app")
}

if !app.activate() {
  dbg("activate returned false for pid \(targetPid)")
}

let deadline = Date().addingTimeInterval(1.0)
while Date() < deadline {
  if NSWorkspace.shared.frontmostApplication?.processIdentifier == targetPid {
    break
  }
  usleep(20_000)
}
guard NSWorkspace.shared.frontmostApplication?.processIdentifier == targetPid else {
  emit(false, "activation_failed")
}

let axOptions = [kAXTrustedCheckOptionPrompt.takeRetainedValue() as String: true] as CFDictionary
guard AXIsProcessTrustedWithOptions(axOptions) else {
  emit(false, "accessibility_denied")
}

guard let source = CGEventSource(stateID: .hidSystemState),
      let vDown = CGEvent(keyboardEventSource: source, virtualKey: 0x09, keyDown: true),
      let vUp = CGEvent(keyboardEventSource: source, virtualKey: 0x09, keyDown: false) else {
  emit(false, "event_create_failed")
}
vDown.flags = .maskCommand
vUp.flags = .maskCommand
vDown.post(tap: .cghidEventTap)
// WKWebView-цели (Tauri) теряют вставку, если keyUp идёт вплотную за
// keyDown — подтверждено repro: <5мс глотается, ≥5мс работает.
usleep(20_000)
vUp.post(tap: .cghidEventTap)
emit(true)
