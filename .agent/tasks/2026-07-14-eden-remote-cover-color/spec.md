# Eden remote cover color

## Goal

Use the dominant color of a remote book cover for Eden's physical-cover shell instead of the gray fallback when browser canvas access is blocked by CORS.

## Scope

- Eden-only internal preload IPC that returns a color, never remote bytes.
- HTTPS PNG/JPEG covers with public-network DNS resolution, pinned lookup, redirect validation, byte and pixel limits.
- Existing renderer canvas remains the first path; remote IPC is only its CORS fallback.
- No ARK/schema/sync/API changes, dependencies, release work, or persisted derived color.

## Frozen acceptance criteria

- **AC1.** The real dev record `Марафон в рай` resolves to a non-gray image-derived color.
- **AC2.** Local/private/reserved addresses and credentials are rejected, including every redirect target; the validated DNS result is pinned to the request.
- **AC3.** Unsupported/malformed/oversized images return `null` before native decoding where dimensions can be inspected.
- **AC4.** The renderer ignores stale async results and retains the gray fallback when extraction fails.
- **AC5.** Targeted unit, Eden browser component, desktop build, visual regression, and independent review pass.
