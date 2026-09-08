# Open evidence problems

Status: `OPEN` — this file records unavailable or partial criteria; it does
not authorize cleanup or legacy removal.

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

## AC16 — PASS_WINDOWS_ONLY / umbrella blocked by deferred scope

Final merged Core commit `ba13c5b5aed364ef1b6704b5e08a98bc1898964a`, Cortex
`199a4e379c5423c2e9b87cc46d000a0fefd53704` and Engine v0.1.2 are pinned. The
umbrella remains blocked only by user-deferred non-Windows/visible-GUI rows;
hosted CI was not started because of the account billing/spending limit.

## AC8 / AC9 — current status

AC8's Windows Core boundary fixture passes 1/1. AC9's Core-owned profile,
typed tombstone path and actual two-DB selective-sync transport matrix pass on
Windows. Non-Windows verification remains deferred by user; no new transport
or negotiation protocol is claimed beyond the frozen local-profile contract.

## Deferred scope

Debian/Linux/macOS and visible GUI verification remain `DEFERRED_BY_USER` /
`NOT_RUN`; they are not represented as PASS in the Windows-only delivery.
