# Open evidence problems

Status: `OPEN` — this file records unavailable or partial criteria; it does
not authorize cleanup or legacy removal.

## AC1 — PASS_WINDOWS_ONLY

The exact-head Windows evidence covers Core guards, Cortex Host/process/
authority targets, the Arca SDK reconnect contract (`3/3`), and signed Agenda
Host↔Engine restart (`1/1`). Visual GUI and non-Windows rows remain deferred.

## AC10 — NOT_RUN / OPEN

The six frozen production behaviors are now matrixed. Manifest-v1 grants,
canonical registration, HTTP/WS parity, Host authority policy, and Engine
discovery are PASS/guarded; the protocol 30-day condition is PASS. The
immutable Phase 3–8 manifest and isolated backup/reopen/restore counts,
digests, malformed-input rejection, and idempotent rerun proof are present.
All six frozen Windows production behaviors and the literal 30-day protocol
condition pass. However, frozen AC10 explicitly requires removal of legacy
planning writes/tables after migration evidence. AC1/AC2 explain why current
readers/recovery remain necessary but do not override that requirement. A
replacement migration/recovery contract, consumer/source guards and reviewed
deletion proof are still required. `cleanupAuthorized` remains false.

## AC16 — PARTIAL

Independent Docs verification passes against final candidate `ede4e8c3` and is
recorded in `docs-independent-review-20260907.md`. AC16 remains partial because PR
#73 is not merged and the post-merge issue evidence is absent.

## AC8 / AC9 — current status

AC8's Windows Core boundary fixture passes 1/1. AC9's Core-owned profile,
typed tombstone path and actual two-DB selective-sync transport matrix pass on
Windows. Non-Windows verification remains deferred by user; no new transport
or negotiation protocol is claimed beyond the frozen local-profile contract.

## Deferred scope

Debian/Linux/macOS and visible GUI verification remain `DEFERRED_BY_USER` /
`NOT_RUN`; they are not represented as PASS in the Windows-only delivery.
