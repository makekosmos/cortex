# Raycast manifest export cleanup

Remove only safe internal type exports from `platform/desktop/electron/raycast/manifest.ts`.

Keep `RaycastPackageManifest` and `parseRaycastPackageManifest` exported.
Do not change runtime behavior or test seams.
