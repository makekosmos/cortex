# Distribution / bump / git запреты

::: tip Узкий файл
Читайте только когда задача касается этой области. Полный legacy reference: `docs-site/agents/forbidden.md`.
:::

### Distribution

См. [Distribution](/concepts/distribution).

- ❌ Коммитить `GH_TOKEN` (или любой PAT) в repo. Если случайно — rotate immediately.
- ❌ Bundle'ить extensions в Kepler installer (`platform/desktop/package.json → build.extraResources`). Lean installer — marketplace flow обеспечивает установку. Нарушение → лишний размер инсталлера + рассинхрон версий extension'ов между installer'ом и marketplace.
- ❌ Push release tag в `makekosmos/desktop` или `makekosmos/extensions` manually без `electron-builder publish` (launcher) / `ext:publish` (extensions). Эти скрипты генерируют `sha256` + `latest.yml` — autoUpdater сломается без них.
- ❌ Менять wire format `catalog.json` без bump `schemaVersion`. Installed Kepler'ы должны продолжать читать старый format (tolerant к unknown fields).
- ❌ Удалять published GitHub releases retroactive. Installed Kepler'ы (или offline users) могут пытаться downgrade / re-install; ломается trust в URL'ы из cached catalog.json.
- ❌ Менять release/update channel без обновления `platform/desktop/package.json → build.win.publish` / `build.mac.publish` + `extension-marketplace.ts → CATALOG_URL` + `publish-extension.mjs → RELEASES_REPO` + `generate-catalog.mjs → RELEASES_REPO`. Desktop providers — `makekosmos/desktop` (win) и `makekosmos/desktop-mac` (mac); yoso как канал десктопа больше не используется.

### usage-tracker

- ❌ Превращение в Windows Service.
- ❌ Добавление UI / tray icon / окон.
- ❌ Прямой SQL write без `ark_core::db` хелперов и без обновления `version_vector`.
- ❌ Возврат standalone-бинарника по пути services/usage-tracker. После Phase E3 старый usage-tracker больше не active-tree path, а активный код живёт как модуль `platform/runtime/src/usage_tracker/`.

## Файловые операции на Windows

::: danger Junction'ы bun workspaces
В этом репо `bun install` создаёт junction'ы (Windows-симлинки) в `platform/desktop/node_modules/@kepler/<pkg>` → `packages/<pkg>` и аналогично в `extensions/<id>/node_modules/`. PowerShell `Move-Item -Force` (и многие GUI-операции) **разрешают** junction'ы и удаляют **таргет** вместе с источником — а Корзину минуют. Так уже было потеряно несколько часов untracked-работы в `packages/visuals/` до brand swap. Восстановление возможно только если файлы успели попасть в asar предыдущего билда.
:::

- ❌ `Move-Item -Force` или `Remove-Item -Recurse -Force` на `apps/<name>/` целиком, пока внутри есть `node_modules/`. Сначала **удали** `apps/<name>/node_modules/` (`Remove-Item -Recurse -Force apps\<name>\node_modules`), и **только потом** перемещай или удаляй директорию.
- ❌ Переименование/перемещение `apps/<name>/` без предварительной коммитной зачистки untracked-файлов в `packages/*`. Если что-то ценное лежит как `??` в `git status` — закоммить или временно сохрани вне репо, иначе `Move-Item` уничтожит таргет junction'а навсегда.
- ❌ Удаление любых директорий внутри `apps/` или `packages/` через GUI-проводник Windows. Используй `git rm`, `Remove-Item` после удаления `node_modules`, или CLI с явным контролем.
- ❌ `rm -rf packages/...` или эквиваленты, если результат можно достичь через `git restore` / переключение веток.

## Bump / release

- ❌ **Бамп версии без явного запроса пользователя.** Никогда не делать bump «попутно с фиксом», «логически завершить релизом», «раз изменил manifest». Bump = release = пользователи получают update через auto-updater и видят пуш-уведомление. Каждый bump — отдельная команда пользователя («бамп eden», «бамп shell», «релизни»). Edit'ить код, чинить баги, обновлять docs, коммитить, пушить — без bump'а можно и нужно. Менять `manifest.json::version` / `package.json::version` или запускать `ext:publish` / `electron-builder --publish` — **только** по явному запросу. См. [bump skill](/.agents/skills/bump/SKILL.md).
- ❌ Bump'ить чтобы «закрыть тест-цикл» / «убедить юзера что фикс работает». Юзер сам решит когда релизить.

## Git / tooling

- ❌ `--no-verify` при коммите.
- ❌ `git push --force` в `main` / `master`.
- ❌ `git reset --hard` или `git checkout .` для уничтожения чужих изменений.
- ❌ Амендить уже опубликованные коммиты.
- ❌ Коммит файлов с секретами (`.env`, `credentials.json`).
- ❌ Коммит `dist/`, `build/`, `coverage/`, `.tmp/`, `.e2e/`, `node_modules/`.
- ❌ Создание новых правил в `MEMORY.md` или AGENTS.md без согласования с человеком.

## Файловые операции на Windows

::: danger Junction'ы bun workspaces
В этом репо `bun install` создаёт junction'ы (Windows-симлинки) в `platform/desktop/node_modules/@kepler/<pkg>` → `packages/<pkg>` и аналогично в `extensions/<id>/node_modules/`. PowerShell `Move-Item -Force` (и многие GUI-операции) **разрешают** junction'ы и удаляют **таргет** вместе с источником — а Корзину минуют. Так уже было потеряно несколько часов untracked-работы в `packages/visuals/` до brand swap. Восстановление возможно только если файлы успели попасть в asar предыдущего билда.
:::

- ❌ `Move-Item -Force` или `Remove-Item -Recurse -Force` на `apps/<name>/` целиком, пока внутри есть `node_modules/`. Сначала **удали** `apps/<name>/node_modules/` (`Remove-Item -Recurse -Force apps\<name>\node_modules`), и **только потом** перемещай или удаляй директорию.
- ❌ Переименование/перемещение `apps/<name>/` без предварительной коммитной зачистки untracked-файлов в `packages/*`. Если что-то ценное лежит как `??` в `git status` — закоммить или временно сохрани вне репо, иначе `Move-Item` уничтожит таргет junction'а навсегда.
- ❌ Удаление любых директорий внутри `apps/` или `packages/` через GUI-проводник Windows. Используй `git rm`, `Remove-Item` после удаления `node_modules`, или CLI с явным контролем.
- ❌ `rm -rf packages/...` или эквиваленты, если результат можно достичь через `git restore` / переключение веток.
