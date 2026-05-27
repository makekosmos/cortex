# Problems

## P1 — Native contract test read the wrong command source

`bun run test:e2e -- --grep "extension contract: akasha"` initially failed
because the native branch asserted against `window.kepler.__test.getStats()`.
That debug helper reports backend dynamic commands, while Akasha's
`akasha:open` is manifest-declared and resolved by shell-side
`window.kepler.commands.list()`.

Fix: update the native contract branch to assert against
`window.kepler.commands.list()`.

Reverify: the same command passed after the fix.
