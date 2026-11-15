# The dictation pill's delay after the hotkey

Measured 2026-11-15 on Windows 11, 165 Hz display, Engine 0.11.0, the pill as a
separate component (`cortex/plugins/dictation`). The complaint was one the eye
catches immediately: press the hotkey, and the pill shows up "through molasses".

## How it was measured without touching the keyboard

The global hotkey cannot be pressed in a session that must not take the user's
focus, and the RPC `dictation.toggle` sends exactly the event the hook sends
(`dictation.trigger`, kind `toggle`), so it is the same path from the Engine
onwards. The pill writes every show and hide to
`%LOCALAPPDATA%\Mundus\logs\dictation.log` with epoch milliseconds, so:

- trigger → first frame = Engine side (trigger, capture start, first broadcast);
- first frame → pill shown = component side (window, GPU, first present).

`.dev-logs/pill-latency.sh` runs three sessions in a row and prints both.

## What it was before

| session | trigger → first frame | trigger → pill shown |
|---|---|---|
| 1 | 220 ms | 1333 ms |
| 2 | 237 ms | 1428 ms |
| 3 | 162 ms | 1205 ms |

The Engine was already fast: `op_start_recording` sets the state and starts the
frame thread *before* `native_capture::start` opens the microphone, so frames
reach the plugin in ~a tenth of a second. The second was all in the plugin:
`gpui::App::open_window` — creating the window, first compose, first present —
costs **~1.1 s on Windows**, every time, not only the first time in a process.

## What was changed

1. **The window is created once and kept.** The plugin builds it at startup,
   blank and shapeless, and a session only fills it (image) and shapes it
   (region); the end of a session hides it with `SW_HIDE` and an empty region.
   Between sessions the window still exists, but it is not visible and not
   hit-testable, and nothing is drawn because nothing notifies it.
2. **The idle poll went from 120 ms to 25 ms.** The plugin's UI loop samples the
   frame slot; at 120 ms a third of the remaining latency was just the wait.
3. `SW_SHOWNA` (show, do not activate) is what brings it back, so the pill never
   takes the focus from what the user is typing into.

## After

| session | trigger → first frame | trigger → pill shown |
|---|---|---|
| 1 (cold process) | 838–912 ms | 1972–2240 ms |
| 2 | 89–99 ms | 123–128 ms |
| 3 | 86–102 ms | 120–130 ms |

Session 1 is cold because the harness had just replaced the plugin's binary and
killed the process, so the Engine had to launch it — that path is the old
startup cost and it is why the Engine keeps the pill process warm
(`plugins::pill::ensure_running` at Engine start, not at the first session).

## Verifying the window is gone when it should be

`EnumWindows` + `IsWindowVisible` over the plugin's process shows one visible
window (the plate plus its shadow margin) while recording and none after the
session ends; `GetWindowRgn` is not usable for this, it reports "no region" for
composed popups either way.

## Trap: the `/v1/rpc` body is flat

`DispatchRequest::from_wire` (runtime/crates/engine-base/src/engine_dispatch.rs)
removes only `operation` and `id` and takes **the rest of the object** as
`params`. So a hand-written call is

```json
{ "operation": "engine.autostart.set", "enabled": true }
```

and not `{ "operation": …, "params": { "enabled": true } }` — the nested form
becomes `params.params`, every handler reads it as an empty object, and a switch
that takes a boolean silently takes `false`: the autostart call answered
`ok: true, enabled: false` and rewrote the StartupApproved marker as disabled
while leaving the Run value alone. The Manager itself flattens params into the
body (`Engine::rpc`), which is why the UI path never showed this.
