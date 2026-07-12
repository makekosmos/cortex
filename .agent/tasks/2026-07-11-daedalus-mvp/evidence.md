# Evidence

## Result

Daedalus MVP реализован как first-party Vue/Electron extension с лениво создаваемым Rust `agents` service внутри `kepler-backend`. Runtime проверен как protocol-strict fake app-server, так и настоящим установленным Codex CLI.

## Acceptance evidence

- Extension: `products/daedalus/`, production-relative Vite assets, secure directory picker, permissions и команда `daedalus:open`; universal contract зелёный.
- Runtime: обязательный `initialize` → `initialized`, JSONL app-server, `thread/start|resume`, `turn/start|steer|interrupt`, model list, approvals, request-user-input и awaited shutdown всех child processes.
- Modes: policy передаётся на каждый turn; default, auto-review и full-access имеют отдельные sandbox/approval/reviewer параметры.
- Recovery: event sequence хранится в SQLite; snapshot возвращает backlog после `afterSeq`; subscribe начинается до hydrate; strict fake test доказывает restart/resume, stale approval expiry и follow-up.
- Persistence: projects, sessions, normalized timeline и approvals хранятся в extension-scoped SQLite; service и DB создаются только при первом `agents.*` RPC.
- Worktrees: dirty base не протекает в worktree; parallel worktrees и полный diff покрыты тестами; removal разрешён только для archived, inactive и clean worktree, без force.
- Safety: allowlist редакторов, symlink/path containment для diff, чтение только первых 8 KiB для binary detection, 1 MiB truncation и RPC timeouts.
- UI: parallel sessions, statuses/attention, virtual timeline + pagination, approval/question cards, Changes tree, editor picker, archive и confirmed worktree removal.

## Verification

- Rust agents suite: 10 passed, 1 ignored opt-in smoke.
- Fake restart/resume integration: passed.
- Real Codex smoke (`codex-cli 0.144.0`): passed за 47.64 с; настоящий app-server создал ожидаемый файл в отдельном temporary worktree.
- Daedalus Vitest: 8/8 passed.
- Daedalus, ARK и desktop typechecks: passed.
- Desktop production JS + all extensions build: passed.
- ARK write-boundary guard: passed.
- Daedalus extension contract: 1 passed.
- Strict headless Electron E2E: 1 passed за 26.8 с — parallel sessions, active-window reopen, approval persistence, Interrupt, Changes, allow, auto-review question/options и final reopen.
- Documentation freshness: passed.

## Visual artifacts

- `.tmp/visual/2026-07-11-daedalus-mvp/daedalus-1280x800-start.png`
- `.tmp/visual/2026-07-11-daedalus-mvp/daedalus-1280x800-session.png`
- `.tmp/visual/2026-07-11-daedalus-mvp/daedalus-900x600-session.png`

Финально перегенерированы headless Chromium после UI-изменений и вручную просмотрены: русский UI, sidebar/архив, timeline, approval card, Changes tree/diff, editor picker и minimum 900×600 читаемы; console/page errors отсутствуют.

## Not run / out of scope

- Installer/release packaging не запускался; production desktop JS и bundled-extension pipeline прошли.
- Android/Iroh остаются следующей фазой согласно scope.
