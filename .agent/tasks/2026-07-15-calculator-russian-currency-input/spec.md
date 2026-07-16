# Russian currency input and compact source expression

## Goal

Accept a compact Russian currency phrase such as `900 долларов в рублях` and show only the source amount (`900 USD`) on the left side of the calculator card.

## Scope

In scope:

- normalization of the four-token form `<amount> <source currency> в <target currency>`;
- common Russian aliases for USD, RUB, EUR, and CNY;
- compact source-only expression for both normalized Russian input and ISO forms such as `100 USD to RUB`;
- preserving the original search input and existing result/copy behavior.

Out of scope:

- general Russian mathematical natural-language parsing, arbitrary word order, spelled-out numbers, and grammatical generation.

## Acceptance criteria

**AC1.** `900 долларов в рублях` is normalized to `900 USD to RUB` before rate lookup and evaluation, while ordinary calculations are unchanged.

**AC2.** The calculator RPC returns the display expression separately from the result. Currency conversions use only the source side (`900 USD` or `100 USD`); non-currency calculations keep their full original expression.

**AC3.** The launcher card renders the RPC-provided display expression and continues to update atomically without flicker.

**AC4.** Rust tests, desktop typecheck/build, launcher E2E, formatting, and visual verification pass against the final worktree.
