# AGENTS.md — Cortex

Cortex owns the desktop packaging (`desktop/`), Manager (`manager-gpui/`) and
the Rust runtime (`runtime/`). The `makekosmos/docs` repo is retired —
cross-cutting documentation lives in-tree under [`docs/`](docs/).

## Карта

- `runtime/` — Mundus Engine (Rust): Engine API (`/v1/rpc` + WS), package
  host/supervisor, `focus.*`/`pomodoro.*` ops, dictation, updater, tray.
  In-process ARK host: `runtime/src/ark_host.rs`.
- `core/crates/ark-core/` — vendored ARK runtime (Rust + SQLite, sync).
  Contract and invariants: `core/crates/ark-core/AGENTS.md`,
  `docs/ark-core.md`, `docs/sync.md`, `docs/write-boundary.md`.
- `manager-gpui/` — GPUI shell app (dashboard, settings, data browser).
- `desktop/` — installer/packaging and release pipeline scripts.
- `scripts/` + `docs/` — local gate implementation and architecture notes.
- Focus engine ops live in `runtime/src/focus.rs`/`pomodoro*` (the focus UI
  lives in ordo — see `docs/focus.md`).

## Universal never rules

- ❌ GitHub Actions workflows (`.github/workflows/**`) — орг не оплачивает hosted CI; локальные lefthook-гейты — единственные проверки. Не добавлять и не чинить workflows.
- ❌ `git add -A`, `--no-verify`, `git reset --hard`, force-push в main/master.
- ❌ Попутный рефакторинг; один логический change — один коммит.
- ❌ Объявлять PASS без релевантных checks.

## Проверки

- `pnpm run check` — полный локальный гейт; `pnpm run check:affected` — по изменённым файлам.
- Гейты делят кэш по хэшу дерева на диске: прогнав `check:affected` (или `check`) на неизменном дереве, коммит и `git push -u` пройдут по кэшу за секунды. Любая правка после прогона инвалидирует кэш — не редактируй файлы между check:affected и push.
- `pnpm run check:brand` — brand gate (KOS-266): `kosmos`/`kepler` references must be `MIGRATION(KOS-267)`-marked or in `scripts/brand-allowlist.json` (see `docs/brand-legacy-identifiers.md`).
- Хуки: `pnpm exec lefthook install`; pre-commit/pre-push гоняют `check:plan --run`. Pre-commit — только быстрые проверки (секунды), clippy/Rust-тесты/staging/Manager откладываются до pre-push. Rust-тесты идут через `cargo nextest` (нужен `cargo install cargo-nextest --locked`).
- `node scripts/check-core-pin.mjs` — консистентность пина ark-core.

## Agent edit loops (KOS-332)

- В цикле «правка → typecheck → тесты» НЕ гонять `--workspace --all-targets`.
  Скоупить по изменённым крейтам:
  - `cargo check -p <crate> --all-targets` (для `engine` добавить
    `--features engine/package-worker-fixture,engine/markdown-bridge-fixture`,
    как у гейта).
  - `cargo nextest run -p <crate> [filter]` — фильтр по имени теста.
  - Workspace members: `engine` (= `runtime/`), `package-protocol`,
    `pe-version-info`, `ark-core` (= `core/crates/ark-core`). `manager-gpui` —
    отдельный workspace: `cargo check --manifest-path manager-gpui/Cargo.toml`.
- `node scripts/quick-check.mjs` — мапит изменённые файлы (git status) на
  крейты и запускает `check` + `nextest` только для них. Опции:
  `--check-only`, `--files <path>...`, позиционные аргументы — фильтр nextest.
- Типичная итерация — десятки секунд на прогретом кэше (мелкий крейт ~5 s).
- `local-dictation` (KOS-337): локальный dictation backend (whisper.cpp +
  Parakeet ONNX через `transcribe-rs`/`ort`) собирается только с фичей
  `engine/local-dictation` (= `engine-dictation/local-dictation`). Дефолтные
  dev/agent сборки — БЕЗ неё (минус ~31 крейт ONNX-стека); `local::transcribe`
  и `preload_server` тогда возвращают
  `LocalError::NotBuiltWithLocalDictation`, groq/cloud не затронут. Полный
  гейт и release собирают С фичей — правки в `dictation/local/*` проверять
  `cargo check/nextest -p engine-dictation --features local-dictation`.
- mbx уже шарит кэш компиляций между worktree'ми (`mbx[cache]: N hits` в
  выводе cargo) — НЕ делать `cargo clean` и не задавать свой
  `CARGO_TARGET_DIR`: это ломает shared cache и форсит холодную пересборку.
- Полный гейт обязателен перед `git push` (KOS-270: integration-тесты,
  clippy --workspace, fixture bins и staging живут только там). Порядок:
  узкие итерации → `pnpm run check:affected` на финальном дереве → коммит →
  push (pre-push прогонит полный гейт по кэшу дерева). Не редактировать файлы
  между check:affected и push — правка инвалидирует кэш.
