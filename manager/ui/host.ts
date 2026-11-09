/**
 * The host's own window: things the pages ask for and only the shell can do.
 *
 * A press that drags the window is one of them. The pages cannot start a move
 * themselves — `windowDrag` answers the drag rectangles NSH hit-tests while it
 * owns a window, and inside a host (`manager.exe`) that mechanism is inert, so
 * NSH stops the press and does nothing. What works is the host's own Win32
 * call (`manager/src/chrome.rs`), reached through the bridge.
 */
let command: ((name: string) => void) | undefined;

export function connectHost(send: (name: string) => void) {
  command = send;
}

export function dragWindow() {
  command?.("window.drag");
}
