# Knip Workspace Entry Vs Project

## Trigger

Knip/desloppify reports false `DEAD_FILE` or `DEAD_DEPENDENCY` findings in a workspace after adding or changing `workspaces.<name>.entry` or `project`.

## Symptom

Adding broad `project` globs fixes dependency false positives but introduces new unused-file or export noise; Electron entry files, preloads, and build hooks are reported dead, or their packages are reported unused.

## Do This

Test temp configs under `.tmp`; compare current vs candidate with `rtk bunx knip -c <config> --workspace <name> --include dependencies,files,exports,unlisted --reporter compact --no-progress`. Prefer adding real runtime and build entrypoints, such as preloads or `afterPack` hooks, before broadening `project`. Only edit `knip.json` after targeted false positives disappear, file/export/unlisted noise does not materially worsen, and a full desloppify scan keeps or improves the score.

## Avoid

Avoid broad `src/**/*` project expansion without a full comparison. Avoid deleting dependencies just because Knip missed Vue, CJS, or config usage. Avoid trusting only `--include dependencies` when files and exports get noisier. For packages that are not declared as root workspaces, adding their Vite config as a root entry can turn package-local dev deps into root `UNLISTED_DEPENDENCY` findings; reject that even if `DEAD_FILE` drops.

## Promote To Skill When

This repeats across more workspaces or a standardized repo Knip config workflow is needed.
