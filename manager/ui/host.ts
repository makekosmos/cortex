import { titleBar } from "nsh/desktop";

/**
 * The host's own window: things the pages ask for and only the shell can do.
 *
 * A press that drags the window is one of them, and *who* moves it depends on
 * whose window it is. The dev host (`nsh dev`) owns its window — it draws the
 * caption buttons, reports their strip in `titleBar().right`, hit-tests the
 * `windowDrag` areas itself and lets Windows move the window from them. An
 * embedding host (`manager.exe`) reports nothing reserved and swallows the
 * press in its surface before Windows can act on it (`Window::start_window_move`
 * is an empty default on Windows), so the press has to ask the host's own Win32
 * code over the bridge.
 */
let command: ((name: string) => void) | undefined;

export function connectHost(send: (name: string) => void) {
  command = send;
}

/** A press here moves the window, by whichever of the two means applies. */
export function dragBinding(): { desktop?: object; onPressDown?: () => void } {
  return titleBar().right > 0
    ? { desktop: { windowDrag: true } }
    : { onPressDown: () => command?.("window.drag") };
}
