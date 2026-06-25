# kepler-api dead export cleanup

Remove only safe dead exports from `platform/desktop/electron/kepler-api.ts`.

Target names:

- `SemVer`
- `parseSemver`

Keep `KEPLER_API_VERSION` and `satisfiesSemver` exported.
