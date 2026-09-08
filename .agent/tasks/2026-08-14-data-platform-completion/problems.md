# Open evidence problems

Status: `PASS_WINDOWS_ONLY` — this file records only explicitly deferred
rows and one pre-existing release-fixture residual; it does not authorize
destructive cleanup beyond the merged AC10 changes.

## AC1 — PASS_WINDOWS_ONLY

The exact-head Windows evidence covers Core guards, Cortex Host/process/
authority targets, the Arca SDK reconnect contract (`3/3`), and signed Agenda
Host↔Engine restart (`1/1`). Visual GUI and non-Windows rows remain deferred.

## AC3–AC7 — PASS_WINDOWS_ONLY

The packaged Windows headless flow and targeted contracts now close the prior
artifact gaps: v2 install/recovery/uninstall, denied grants and broker fencing,
Store/Package Index separation and replay checks, Marketplace contracts, and
bridge identity/collision/round-trip behavior. Exact commands and pins are in
`raw-windows-headless-20260908.md`.

## AC10 — PASS_WINDOWS_ONLY

The six frozen production behaviors are now matrixed. Manifest-v1 grants,
canonical registration, HTTP/WS parity, Host authority policy, and Engine
discovery are PASS/guarded; the protocol 30-day condition is PASS. Core
planning retirement and Cortex typed migration recovery are implemented and
covered by the focused tests, packaged smoke and Engine 0.1.2 artifact proof
in `raw-ac10-proof-20260908.md`. AC10 therefore passes for the Windows scope.
Non-Windows and visible-GUI rows remain deferred by user.

## AC16 — PASS_WINDOWS_ONLY

Final merged Core commit `ba13c5b5aed364ef1b6704b5e08a98bc1898964a`, Cortex
`199a4e379c5423c2e9b87cc46d000a0fefd53704`, Desktop `v0.9.22` and Engine
`v0.1.2` are pinned. Non-Windows, visible-GUI, autostart, physical-microphone
and Groq rows are explicitly deferred by the user; hosted CI was not started
because of the account billing/spending limit.

## Desktop first-party release fixture residual

The Desktop `v0.9.22` release-contract run passes five of six existing
first-party contracts. The Memoria crash-boundary fixture still times out
waiting for its pre-crash `window.__memoriaImportFirstWrite` marker. This is a
pre-existing fixture residual outside the AC10 typed migration-recovery path;
the packaged AC10 smoke, Engine distribution/install checks and all other
first-party contracts pass.

## AC8 / AC9 — current status

AC8's Windows Core boundary fixture passes 1/1. AC9's Core-owned profile,
typed tombstone path and actual two-DB selective-sync transport matrix pass on
Windows. Non-Windows verification remains deferred by user; no new transport
or negotiation protocol is claimed beyond the frozen local-profile contract.

## Deferred scope

Debian/Linux/macOS and visible GUI verification remain `DEFERRED_BY_USER` /
`NOT_RUN`; they are not represented as PASS in the Windows-only delivery.
