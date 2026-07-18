# Codewars integration

## Goal

Add Codewars as a first-class Kosmos integration and show its locally imported statistics on the existing Coder page.

## Scope

- Connect a public Codewars profile by username through the existing integrations settings flow.
- Import the profile and completed kata through the official public Codewars API.
- Persist imported records through existing ARK RPC upserts with stable source IDs.
- Reuse the existing Coder tabs, summary, activity, breakdown, and recent-items UI.
- Keep LeetCode behavior and cached navigation unchanged.

Out of scope: failed Codewars attempts and per-kata difficulty distribution, because the public completed-challenges endpoint does not expose them and deriving them would require one extra request per kata.

## Acceptance criteria

**AC1.** The integrations page contains a connected Codewars tile and settings modal where a username can be verified, saved, replaced, removed, scheduled, and synchronized manually.

**AC2.** First synchronization imports every completed Codewars kata; later synchronization stops after the last successful overlap window. ARK objects use stable IDs, so repeat synchronization updates without duplicates.

**AC3.** The Coder page switches between LeetCode and Codewars without refetch flicker and shows only the selected platform's local summary, streaks, activity, language breakdown, profile metrics, and recent kata.

**AC4.** Codewars kata links open `https://www.codewars.com/kata/<slug>` and the UI does not claim that unavailable failed-attempt or difficulty data exists.

**AC5.** Rust tests, Coder unit tests, desktop typecheck/build, ARK write-boundary guard, and targeted desktop E2E pass against the final code; the integration and Coder Codewars states are visually inspected.
