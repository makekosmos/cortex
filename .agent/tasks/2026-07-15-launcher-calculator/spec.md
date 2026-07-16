# Launcher calculator

## Goal

Add an inline calculator to the Kosmos launcher using the existing Rust runtime and `fend-core`, so short calculations can be evaluated and copied without opening another application.

## Scope

In scope:

- arithmetic, percentages, unit conversions, number bases, and date expressions supported offline by `fend-core`;
- live result in the existing launcher command list;
- Enter/click copies the result and closes the launcher;
- a bounded input and a quiet `null` result for ordinary search text or invalid expressions.

Out of scope:

- currency and crypto exchange-rate fetching;
- Russian natural-language normalization and localized aliases;
- calculator history, settings, color conversion, and time-zone conversion;
- a separate calculator page or service.

## Acceptance criteria

**AC1.** The Rust runtime exposes `calculator.evaluate` through the existing local WebSocket RPC path. A valid expression returns `{ "result": string }`; ordinary text, invalid expressions, and overlong input return `{ "result": null }` without invoking shell commands or network access.

**AC2.** The evaluator correctly handles representative offline scenarios: arithmetic (`1200 * 1.2`), percentages (`15% of 4500`), unit conversion (`10 km to miles`), and date arithmetic supported by `fend-core`.

**AC3.** While the launcher is in command mode, a valid calculation appears as the first selectable result without replacing normal command/file search. Stale asynchronous responses cannot replace the result for a newer query.

**AC4.** Activating the calculator result copies its plain-text value to the clipboard, closes the launcher, and does not invoke the command bus.

**AC5.** Targeted Rust tests, desktop TypeScript checking, formatting, and a visible launcher verification pass against the final current worktree.
