# Raycast view host vertical slice

## Classification

`FULL_LOOP` — task touches extension command invocation, Electron window lifecycle, preload IPC, shell renderer UI, and Raycast compatibility contracts.

## Goal

Add the first safe Raycast `view` command host for trusted extensions, using Kosmos shell/visuals patterns and without enabling untrusted third-party JavaScript execution in Electron main.

## Scope

In scope:

- Dedicated Raycast view runner for trusted `dev` / `bundled` sources.
- Snapshot normalization for serializable Raycast component trees.
- Shell IPC/preload bridge for fetching a view session snapshot.
- BrowserWindow host route for `#raycast-host`.
- Vue shell renderer for `List`, `List.Item`, selected `Detail` markdown, search filtering, and `ActionPanel` buttons.
- Focused unit tests, shell typecheck, docs update, and visual verification screenshot under `.tmp/visual/`.

Out of scope:

- Untrusted user-installed command sandboxing.
- Full React/TSX runtime, JSX runtime entrypoints, command bundler, HMR.
- Rich Raycast parity for `Form`, `Grid`, `MenuBarExtra`, OAuth, AI, Store distribution, and action execution side effects.
- Publishing/release/version bump.

## Acceptance Criteria

**AC1.** Raycast `view` commands declared in `package.json` invoke through a dedicated trusted-source runner and return a host-renderable snapshot.

**AC2.** User-installed Raycast `view` command code is refused before execution, matching the no-view security gate.

**AC3.** Shell Electron opens a guarded Raycast host window for `raycast-view`, stores/retrieves snapshots by session id, and exposes read-only snapshot IPC through preload.

**AC4.** Shell renderer route `#raycast-host` displays `List` commands with sections, empty view fallback, search, selection, selected detail markdown, and action buttons using Kosmos visuals/layout conventions. `Action.CopyToClipboard`, `Action.OpenInBrowser`, and `Action.Open` execute through guarded session IPC, and `Action.Push` can display its target detail.

**AC5.** Markdown rendering is safe: no raw `v-html`; headings, paragraphs, lists, and fenced code are rendered as Vue text nodes.

**AC6.** Verification evidence records unit tests, `bun run shell:typecheck`, docs sync/check, ARK guards/smoke, and a visual screenshot path or records raw environment failures.
