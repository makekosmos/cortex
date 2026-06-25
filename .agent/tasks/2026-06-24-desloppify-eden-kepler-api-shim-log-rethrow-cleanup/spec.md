# Desloppify Eden Kepler API Shim

Remove the redundant warning from the `ensureTaskObjectTypeRegistered` catch path in `products/eden/src/lib/kepler-api-shim.ts`.

Keep the retry behavior unchanged:

- reset `taskObjectTypeRegisterPromise` to `null`
- rethrow the original error

Scope is limited to the single catch block in `ensureTaskObjectTypeRegistered`.
