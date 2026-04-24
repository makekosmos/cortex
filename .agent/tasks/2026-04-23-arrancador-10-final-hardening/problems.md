# Problems

## Verification pass 1

Result: FAIL

### P1: Typecheck failure in new scan component test

`src-vue/test/scan-results-list.test.ts` used `Array.prototype.at`, but the project TypeScript target does not include that API.

Fix: use explicit array indexing.

### P2: Component tests used stale mojibake expectations

The new component renders readable UTF-8 output through the Vue test environment, so the expectation should assert readable text.

Fix: update expected playtime text.

### P3: Encoding guard caught new component copy/paste text

The extracted components copied existing mojibake Russian strings from files that predate the guard.

Fix: replace text in the new component files with proper UTF-8 Russian strings.
