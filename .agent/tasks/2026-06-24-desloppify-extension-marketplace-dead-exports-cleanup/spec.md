# Extension marketplace dead export cleanup

Scope: `platform/desktop/electron/extension-marketplace.ts`

Goal: remove safe `DEAD_EXPORT` runtime exports for `fetchCatalog`, `installFromUrl`, and `autoUpdateExtensionsOnce` without changing behavior or touching the shared `Catalog` / `CatalogExtension` type exports.

Result: the three runtime exports were de-exported; registration, IPC handlers, and shared type exports remain in place.
