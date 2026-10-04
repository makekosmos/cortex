# AGENTS.md — Cortex

Cortex owns the desktop packaging (`desktop/`), Manager (`manager-gpui/`) and
the Rust runtime (`runtime/`). Техническая документация:
[`makekosmos/docs`](https://github.com/makekosmos/docs).

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
- mbx уже шарит кэш компиляций между worktree'ми (`mbx[cache]: N hits` в
  выводе cargo) — НЕ делать `cargo clean` и не задавать свой
  `CARGO_TARGET_DIR`: это ломает shared cache и форсит холодную пересборку.
- Полный гейт обязателен перед `git push` (KOS-270: integration-тесты,
  clippy --workspace, fixture bins и staging живут только там). Порядок:
  узкие итерации → `pnpm run check:affected` на финальном дереве → коммит →
  push (pre-push прогонит полный гейт по кэшу дерева). Не редактировать файлы
  между check:affected и push — правка инвалидирует кэш.
