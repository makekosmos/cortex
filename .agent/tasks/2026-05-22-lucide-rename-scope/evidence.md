# Evidence: lucide-rename-scope

## AC1 — package.json clean

```
$ grep "lucide-vue-next" **/package.json
(no matches)
```

`packages/visuals/package.json::peerDependencies["@lucide/vue"]: "*"` добавлено
взамен старого `lucide-vue-next: "*"` (bun remove стёр строку, восстановили
вручную).

## AC2 — shell build:js

```
$ bun run --cwd shell build:js
✓ built in 1.01s
```

PASS

## AC3 — shell typecheck

```
$ bun run --cwd shell typecheck
$ tsc --noEmit
(exit 0)
```

PASS

## AC4 — oxlint

```
$ bunx oxlint .
(13 warnings, 0 errors — все pre-existing: no-unused-vars,
 no-useless-fallback-in-spread)
```

PASS

## AC5 — oxfmt

```
$ bunx oxfmt --check .
Checking formatting...
All matched files use the correct format.
Finished in 3961ms on 1435 files using 12 threads.
```

PASS

## AC6 — Playwright eden

```
$ bunx playwright test tests/e2e/eden.spec.ts
9 passed (59.1s)
```

PASS

## AC7 — grep clean

```
$ grep -r "lucide-vue-next" --exclude-dir=legacy --exclude-dir=.vitepress \
    --exclude-dir=raw --exclude-dir=node_modules .
(0 matches в активном коде)
```

Совпадения остаются только в:

- `legacy/dashboard-extension/` — frozen код, не трогаем.
- `.agent/tasks/*/raw/*.txt` — захардкоженные исторические diff'ы.
- `.agent/tasks/2026-05-22-major-deps-bump/{spec,evidence}.md` — историческая
  запись предыдущей задачи.
- `.agent/tasks/2026-05-13-delphi-task-ux-overhaul/spec.md` — историческая
  спека.
- `.agent/tasks/2026-04-20-arrancador-react-to-vue-feature-inventory/inventory.md`
  — историческая.
- `docs-site/.vitepress/dist/` — build output VitePress, регенерится при
  следующем `docs:build`.

PASS

## Итого

55 файлов изменено (8 package.json + 2 bun.lock + 3 docs-site .md + 2 build
config файла + 40 .vue/.ts с импортами). 7 workspace'ов bump'нуто. 0 issues.
