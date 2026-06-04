# Spec - Raycast feedback host slice

## Classification

FULL_LOOP. This extends Raycast command runtime feedback through Electron main,
preload IPC, renderer UI, generic action dispatch, docs, and visual
verification.

## Goal

Trusted Raycast-compatible commands can use `showToast`, `showHUD`, and
`confirmAlert`, and generic `Action` callbacks can execute through the guarded
Raycast session registry.

## Acceptance Criteria

- `showToast` and `showHUD` emitted by trusted command callbacks are forwarded
  to the matching Raycast host window.
- The renderer exposes and consumes a narrow `window.kepler.raycast.onFeedback`
  subscription.
- Raycast host renders feedback with Kosmos visual tokens and readable contrast.
- `confirmAlert` delegates to a host-provided native confirmation adapter.
- Declared no-view commands launched by main pass the native `confirmAlert`
  adapter.
- Generic `Action` callbacks dispatch through guarded session IPC in List,
  Grid, and Form hosts.
- Unit tests cover runtime confirm, command-runner confirm, and feedback events
  from view action callbacks.
- Visual verification covers feedback rendering after a generic action click.
- Docs mention the supported boundary.

## Out of Scope

- Raycast toast update APIs and primary toast action callbacks.
- Modal-style custom confirm UI in the renderer.
- Untrusted extension permission prompts.
- Persisted per-extension logs/diagnostics UI.
