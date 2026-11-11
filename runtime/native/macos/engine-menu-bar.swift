import AppKit
import Foundation
import Darwin

NSApplication.shared.setActivationPolicy(.accessory)

func emit(_ command: String) {
    print(command)
    fflush(stdout)
}

final class MenuTarget: NSObject {
    @objc func openInterface(_ sender: Any?) { emit("open") }
    @objc func exitEngine(_ sender: Any?) { emit("exit") }
}

let target = MenuTarget()
let statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
if let button = statusItem.button {
    if CommandLine.arguments.count > 2,
       let image = NSImage(contentsOfFile: CommandLine.arguments[2]) {
        // 18 points is the menu bar's own glyph height; the file is 36 px, so a
        // Retina bar draws it natively. A template image is tinted by AppKit.
        image.size = NSSize(width: 18, height: 18)
        image.isTemplate = true
        button.image = image
    } else {
        button.title = "M"
    }
    button.toolTip = "Mundus Engine"
}

let menu = NSMenu()
let open = NSMenuItem(title: "Открыть интерфейс", action: #selector(MenuTarget.openInterface(_:)), keyEquivalent: "")
open.target = target
menu.addItem(open)
menu.addItem(.separator())
let quit = NSMenuItem(title: "Выход", action: #selector(MenuTarget.exitEngine(_:)), keyEquivalent: "")
quit.target = target
menu.addItem(quit)
statusItem.menu = menu

if let rawParentPid = ProcessInfo.processInfo.environment["MUNDUS_PARENT_PID"],
   let parentPid = Int32(rawParentPid) {
    let watchdog = Timer(timeInterval: 1.0, repeats: true) { _ in
        if kill(parentPid, 0) != 0 && errno == ESRCH {
            exit(0)
        }
    }
    RunLoop.main.add(watchdog, forMode: .common)
}

NSApplication.shared.run()
