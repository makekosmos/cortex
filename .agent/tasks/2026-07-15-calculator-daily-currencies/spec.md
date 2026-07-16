# Daily currency rates for launcher calculator

## Goal

Allow launcher expressions such as `100 USD to RUB` to use official daily exchange rates without making ordinary calculations depend on the network.

## Scope

In scope:

- official daily rates from the Bank of Russia XML endpoint;
- a 24-hour durable cache under the existing runtime data directory;
- stale-cache fallback when refresh fails or the machine is offline;
- currency conversion through the existing `fend-core` evaluator and launcher result card.

Out of scope:

- real-time market quotes, crypto assets, historical-rate queries, multiple providers, settings UI, and background scheduling while Kosmos is not running.

## Acceptance criteria

**AC1.** A query containing a currency conversion (for example `100 USD to RUB`) loads official Bank of Russia rates at most once per 24 hours and passes them to `fend-core`; ordinary arithmetic and unit queries do not perform a rate request.

**AC2.** The cache is persisted in the runtime data directory. A fresh cache is used without network access; if refresh fails, the last valid stale cache remains usable; without any valid cache, the calculator quietly returns no currency result.

**AC3.** Rate parsing accounts for the Bank of Russia `Nominal` field and always exposes RUB at `1.0`, so conversions between RUB and listed ISO currencies are mathematically correct.

**AC4.** Existing arithmetic, percentage, unit, date, stable-card, keyboard navigation, and clipboard behavior continue to pass their targeted checks.

**AC5.** Targeted Rust tests, desktop typecheck/build, formatting, and launcher E2E pass against the final worktree.
