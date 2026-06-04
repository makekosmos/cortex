# Raycast Form host vertical slice

## Classification

`FULL_LOOP` — task touches Raycast command lifecycle, trusted callback execution, preload IPC payloads, shell renderer UI, and compatibility docs.

## Goal

Add the first safe Raycast `Form` root host for trusted extensions, including field rendering and real `Action.SubmitForm` callback execution.

## Scope

In scope:

- Normalize `Form` snapshots, including `actions`.
- Register trusted view callbacks for `Action.SubmitForm` / generic action functions during snapshot normalization.
- Store callback registries per Raycast host session and execute them through guarded session IPC.
- Render `Form.TextField`, `Form.PasswordField`, `Form.TextArea`, `Form.Checkbox`, `Form.Dropdown`, `Form.DatePicker`, and a basic text-backed fallback for file/path-like fields.
- Submit current form values through `Action.SubmitForm`.
- Unit coverage, shell typecheck, docs update, and visual screenshot.

Out of scope:

- Full Raycast form validation/drafts.
- `Action.Paste`, `Form.FilePicker` native picker behavior, multi-select, and rich date objects.
- Untrusted user-installed JS sandboxing.
- Full React/TSX runtime and bundler.
- Publishing/release/version bump.

## Acceptance Criteria

**AC1.** `Form` root snapshots render in `#raycast-host` instead of falling through to unsupported view.

**AC2.** Form field models include ids, titles, placeholders, defaults, checkbox values, and dropdown options.

**AC3.** `Action.SubmitForm` callbacks from trusted commands are registered in the view session and invoked through guarded session IPC with current form values.

**AC4.** Renderer Form UI uses Kosmos visual tokens, has Russian fallback UI text, and avoids raw HTML rendering.

**AC5.** Verification evidence records unit tests, `bun run shell:typecheck`, visual screenshot, docs sync/check or any environment limitation with raw context.
