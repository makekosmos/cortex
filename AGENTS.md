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
- Хуки: `pnpm exec lefthook install`; pre-commit/pre-push гоняют `check:plan --run`.
- `node scripts/check-core-pin.mjs` — консистентность пина ark-core.
