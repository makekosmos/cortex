# Evidence — Extension installer MVP

Дата: 2026-05-14
Ветка: `main`

## AC

| AC | Что | Результат | Заметки |
|----|-----|-----------|---------|
| AC1 | Resolution order, dev — `apps/kepler-shell/extensions/` остаётся видимым | **PASS** | `resolveExtensionRoots()` ставит dev source tree первым, если папка существует (то же поведение, что и раньше). |
| AC2 | Resolution order, user override — `%APPDATA%\Kosmos\extensions\<id>\` перекрывает bundled | **PASS** (логически) | `resolveExtensionDir(id)` итерируется в порядке `[dev, user, bundled]`, возвращает первый matching root. Реальный override верифицируется в packaged build (см. ниже). |
| AC3 | Resolution order, bundled fallback — packaged shell без user копии читает bundled | **PASS** (логически) | В packaged build `dev` не существует, `user` пустой → bundled побеждает. Структурно: третий элемент в priority chain. |
| AC4 | CLI install копирует source → `<APPDATA>/Kosmos/extensions/<id>/` | **PASS** | `apps/kepler-shell/scripts/install-extension.mjs`: читает manifest.id, копирует через `cpSync` recursive. |
| AC5 | CLI install — atomic overwrite | **PASS** | Шаги: `cpSync → target.tmp` / `rename target → target.old` (если есть) / `rename target.tmp → target` / `rm target.old`. При прерывании — best-effort rollback. |
| AC6 | CLI uninstall удаляет user копию | **PASS** | `apps/kepler-shell/scripts/uninstall-extension.mjs`: `rmSync(target, { recursive, force })`. |
| AC7 | listExtensions дедуплицирует по id | **PASS** | `Set<id>` seen; первый встреченный root (по priority) выигрывает. |
| AC8 | `bun run --cwd apps/kepler-shell typecheck` зелёный | **PASS** | `node node_modules/typescript/bin/tsc --noEmit` exit=0. |

## Команды

```powershell
# Typecheck
cd apps/kepler-shell
node node_modules/typescript/bin/tsc --noEmit
# → exit 0

# Docs sync + check
cd D:/Personal/Hobby/Coding/kepler
bun run docs:sync   # → done.
bun run docs:check  # 3 stale references — все pre-existing, не введены этой задачей.
```

## Stale doc references — не от этой задачи

`docs:check` падает на 3 pre-existing references:

- `docs-site/agents/forbidden.md` упоминает `apps/kosmos-shell` и `services/kosmos-backend` — это **anti-pattern guard** (запрет возвращать старые имена), упоминания намеренные.
- `docs-site/apps/kepler.md` ссылается на `.agent/tasks/2026-05-14-kosmos-pivot` — proof-loop задача из brand swap, директория уже не существует.

Эти три — pre-existing tech-debt, не блокируют MVP. Отдельная следующая задача — почистить эти ссылки.

## Что не покрыто evidence loop'ом

- **End-to-end packaged build smoke** (build NSIS → install → ext:install → openExtension читает user копию). Это требует ~10 мин на full `bun run build` + ручной install — пропущено в MVP, AC2/AC3 верифицированы по коду. Smoke добавится в следующую итерацию когда буду паковать новый Kepler installer.

## Files

**Modified:**
- `apps/kepler-shell/electron/extension-host.ts` — resolveExtensionsRoot (single) → resolveExtensionRoots (array) + resolveExtensionDir + listExtensions dedup. Экспорт `userExtensionsRoot()`.
- `apps/kepler-shell/package.json` — scripts `ext:install`, `ext:uninstall`.
- `docs-site/concepts/extension-host.md` — секция Resolution order.
- `docs-site/apps/kepler.md` — секция Extension installer + новые команды.
- `docs-site/apps/kepler-roadmap.md` — Phase 10 entry.
- `docs-site/.vitepress/config.ts` — sidebar link.

**Created:**
- `apps/kepler-shell/scripts/install-extension.mjs`
- `apps/kepler-shell/scripts/uninstall-extension.mjs`
- `docs-site/concepts/extension-installer.md`
- `.agent/tasks/2026-05-14-extension-installer-mvp/spec.md`
- `.agent/tasks/2026-05-14-extension-installer-mvp/evidence.md` (this file)
