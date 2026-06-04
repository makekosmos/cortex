# 2026-06-04 full review, bump, release — evidence

## Classification

FULL_LOOP. Scope crossed shell permissions, focus widget IPC, extensions,
docs/skills, version bump, build, GitHub Releases, and marketplace catalog.

## Review coverage

- Shell extension permission boundary: reviewed `shell/electron/extension-host.ts`,
  `extension-permissions.ts`, focus widget IPC, settings/autostart flow, and
  related unit/e2e tests.
- Extensions: reviewed Akasha EPUB guardrails, Delphi legacy spaces cleanup,
  extension manifests/package versions, and build output.
- Data/write boundary: reviewed touched ARK client/backend files and ran
  `bun run ark:guard:writes`.
- Docs/agent workflow: reviewed generated `AGENTS.md` rules, docs-site changes,
  `bump` skill, and new `windows-sandbox` skill.
- Release surface: verified extension `.kext` releases, shell installer release,
  `latest.yml`, and marketplace catalog.

## Finding fixed during review

`kepler:focus-widget:set-state` had been moved behind strict extension sender
validation. That blocked shell-owned preload/test-helper calls with
`[kepler-shell] sender is not an extension`.

Fix:

- Added `assertExtensionSenderHostPermissionIfExtension()` in
  `shell/electron/extension-host.ts`.
- Updated `shell/electron/focus-widget.ts` to require the host permission only
  for extension senders while preserving shell/internal callers.
- Updated `tests/e2e/focus-widget-controls.spec.ts` to use a role-based selector.
- Added the regression to `docs-site/agents/postmortems.md`.

## Workflow rules recorded

- `.agents/skills/windows-sandbox/SKILL.md`
  - rerun `windows sandbox: setup refresh failed` commands once with escalation;
  - avoid bash-style `*` globs in PowerShell command arguments;
  - handle locked `target\release\kepler-focus-svc.exe` by building Rust into
    an alternate target dir and pointing electron-builder `extraResources` there.
- `.agents/skills/bump/SKILL.md`
  - default bump is patch-only `+0.0.1`;
  - bump includes build + publish;
  - after electron-builder falls back from missing `GH_TOKEN`, verify/regenerate
    `latest.yml` before manual `gh release create`.

## Verification

Passed:

- `bun test tests/unit/extension-permissions.test.ts tests/unit/settings-autostart-ui.test.ts tests/unit/ark-client-invoke-device-id.test.ts tests/unit/akasha-epub-guardrails.test.ts`
- `bun run --cwd shell typecheck`
- `bun run shell:build`
- `bun run --cwd shell build:extensions`
- `bun run ark:guard:writes`
- `bun run docs:sync`
- `bun run docs:check`
- `bun run ark:smoke`
- `bunx playwright test --config playwright.config.ts tests/e2e/extension-permissions.spec.ts tests/e2e/headless-window-repeat-open.spec.ts tests/e2e/delphi-legacy-cleanup.spec.ts tests/e2e/extensions-contract.spec.ts tests/e2e/commands-architecture.spec.ts tests/e2e/focus-widget-controls.spec.ts`
- `git diff --check`
- `cargo build --release --manifest-path Cargo.toml --target-dir .tmp\cargo-release --bin kepler-backend --bin ark-core-rpc --bin kepler-focus-helper --bin kepler-focus-svc`
- `bun run --cwd shell build:js`

Ran with expected publish fallback:

- `bun run --cwd shell electron-builder --win nsis --publish always --config .tmp\release-builder-config.json`
  produced `Kosmos Setup 0.3.12.exe` and blockmap, then failed only at GitHub
  publish because `GH_TOKEN` is not set. Manual `gh release create` completed
  the release after regenerating `latest.yml`.

Visual evidence:

- `.tmp/visual/2026-06-04-delphi-legacy-cleanup/delphi-main-no-space-setup.png`
  shows Delphi opens to the task UI without legacy SpaceSetup/Spaces onboarding.

Known non-blocking warnings:

- Rust warning: `SubmitError::InjectJoin` is never constructed.
- Node warning during smoke: `DEP0190` for `node:sqlite`.

## Release evidence

Main repo:

- Commit pushed: `224ec2c0 fix(release): harden extensions and bump patch releases`
- Remote: `git@github.com:ksanrse/kosmos.git`

Extension releases in `yoso-industries/kosmos-extensions`:

- `akasha-v0.1.2`: `akasha-0.1.2.kext`, sha256 `a180b15143ab1778fde7ee004482649521277dbbf2085ad2a0fdc77a2ad7d246`
- `arrancador-v0.1.4`: `arrancador-0.1.4.kext`, sha256 `7e1784cf0e508c2b3b05173d3eeafa780204df6fb518ff01878732d9693eb465`
- `delphi-v0.1.7`: `delphi-0.1.7.kext`, sha256 `d7f19392775b4b6c4a68ce6eb7464484616fcbb3df84577ae57c1524d2b538f7`
- `eden-v0.1.12`: `eden-0.1.12.kext`, sha256 `922ccafaa91266a6489881ebc11683a9dfed14c7c2c9fc36974a025a02f6cd2d`
- `horologion-v0.1.7`: `horologion-0.1.7.kext`, sha256 `b927da52ce4f0a9665cfa834864a011e2ef3efacbf59644018e67ad6491ba9c2`

Catalog repo:

- Commit pushed: `490cbd2 chore: refresh extension catalog`
- Remote: `https://github.com/yoso-industries/kosmos-extensions.git`

Shell release in `yoso-industries/kepler-releases`:

- `v0.3.12`, non-draft, non-prerelease.
- Assets verified:
  - `Kosmos-Setup-0.3.12.exe`, sha256 `cf3f503af79fd3abfb1e70ab8e513051115ab7d78a0b29ba3226b72c2e1d90fe`
  - `Kosmos-Setup-0.3.12.exe.blockmap`, sha256 `f153a3035c5c5d715a9544c69c2d9c58a9f785a9239d35fc4c55769b501c78e6`
  - `latest.yml`, sha256 `fe64c2b9e5bec058a8d68c3171c53e49458484e0115a61cd415100e5dfcb3fb6`

## Workarounds used

- Windows Sandbox repeatedly returned `setup refresh failed`; important commands
  were rerun with scoped escalation.
- `KeplerFocusSvc` was installed from workspace `target\release\kepler-focus-svc.exe`
  and could not be stopped without admin service rights. Fresh Rust release
  binaries were built into `.tmp\cargo-release`, and a temporary ignored
  electron-builder config pointed `extraResources` at that target dir.
- electron-builder publish failed without `GH_TOKEN` after producing artifacts.
  Manual `gh release create` was used. Stale `latest.yml` from `0.3.10` was
  replaced with a fresh `0.3.12` file before upload.

## Result

AC1-AC6 pass. No unresolved release blocker remains.
