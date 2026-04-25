# Problems

## P1: `metaJson` type was too broad

The first TypeScript verification failed because `parseMetaJson` returned
`Record<string, unknown>`, which was not assignable to the SDK `JsonValue`
type. The fix was to import `JsonValue` from `@arksync/node` and return that
type.

## P2: Import order

The first focused Biome check reported import ordering issues in the touched
files. The fix was to reorder imports.
