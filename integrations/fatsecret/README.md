# FatSecret integration vertical slice

This directory defines the provider boundary for the FatSecret 3-legged OAuth
1.0a integration. It is intentionally transport- and keyring-injected so all
automated checks use deterministic fixtures and no credentials.

- oauth1.mjs implements RFC 5849 HMAC-SHA1 signing and form/header parsing.
- mapping.mjs maps diary entries to nutrition_entry_obj with deterministic
  fatsecret-food-entry:<external_id> IDs and nullable nutrient omission.
- provider.mjs owns the keyring-only secret contract, OAuth exchange, ARK
  upsertObject boundary, optional recordSync metadata callback, idempotent
  sync, startup/interval/manual lifecycle, disconnect semantics, and a
  per-provider sync lock.
- test/provider.test.mjs uses an injected mock HTTP transport, keyring, and
  ARK writer; it never contacts FatSecret.

The public config contains only provider, clientId, and baseUrl.
Consumer/access secrets are read from the OS keyring adapter and are never
returned in status/config or sent to an ARK object.

Run locally:

    node --test integrations/fatsecret/test/*.test.mjs

This PR does not wire the adapter into Cortex's provider registry because the
old platform/runtime integration layer is no longer present in Core after the
repository split. That wiring, a real callback listener, and live API endpoint
contract should land with the owning Cortex runtime change.
