# Electron Remote Image Color

## Trigger

An extension needs pixels or a dominant color from a remote image that renders in `<img>` but taints browser canvas because the origin does not allow CORS.

## Symptom

The image is visible, `drawImage()` succeeds, but `getImageData()` throws and the UI silently keeps a neutral fallback.

## Do This

- Keep canvas extraction as the first path for data/local/CORS-enabled images.
- Expose a narrow extension-scoped preload IPC that returns only the derived color.
- Fetch only HTTPS through a DNS result pinned to the request; validate every redirect and reject private/reserved addresses.
- Bound total time, response bytes, MIME, and encoded dimensions before decoding with Electron `nativeImage`.
- Verify with the real remote URL in dev, not only a data-URI fixture.

## Avoid

- Do not assume `crossOrigin="anonymous"` can override a server's CORS policy.
- Do not bundle `sharp` into Electron main without proving its native runtime compatibility.
- Do not validate DNS and then let a second resolver choose the connection address.
- Do not use socket-idle timeout as a total download timeout.

## Promote To Skill When

The same fallback is needed by a second extension or for another remote-media operation.
