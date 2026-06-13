# Agent execution protocol

Цель протокола — не «экономить токены», а делать работу правильно: брать ровно тот контекст, который доказывает следующий шаг, менять минимальный участок и проверять релевантным сигналом. Меньше лишнего контекста = меньше регрессий, быстрее итерация, дешевле сессия.

## 0. Классифицируй до чтения больших файлов

| Класс        | Когда                                                                                                      | Рабочий цикл                                                                                   |
| ------------ | ---------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| `NO_LOOP`    | опечатка, одна строка, локальная косметика                                                                 | grep → snippet → edit → быстрая проверка/объяснить почему не нужна                             |
| `LIGHT_LOOP` | один экран/компонент/скрипт, низкий риск                                                                   | grep → snippets → docs only if boundary unclear → edit → targeted check → visual verify для UI |
| `FULL_LOOP`  | новая фича, несколько подсистем, ARK/data/sync/schema/focus/security/command bus, архитектура или сомнение | proof loop → spec/evidence → релевантные docs/skills → tests/guards                            |

Если задача стартовала как `LIGHT_LOOP`, но потребовала читать больше 3 крупных файлов или затронула boundary из `FULL_LOOP`, остановись и эскалируй.

## 1. Context acquisition: вопрос к коду, не чтение «на всякий случай»

Правильный порядок:

1. Сформулируй конкретный вопрос: «где задаётся editable?», «кто рендерит titlebar?», «где пишется ARK объект?».
2. `rtk grep` / `rg` по точным словам, компонентам, CSS class, test id, RPC operation.
3. `read` только 40–120 строк вокруг найденного места.
4. Повтори для следующего вопроса.
5. Читай полный файл только если он маленький или grep не даёт надёжной картины.

Запрещённый паттерн для `NO_LOOP`/`LIGHT_LOOP`: открыть `App.vue`, `Editor.vue`, store и docs «чтобы понять всё». В большом repo это почти всегда ухудшает результат.

## 2. Progressive disclosure для docs и skills

- Для `NO_LOOP` / `LIGHT_LOOP` сначала grep/snippet; routing docs из `claude-md-core.md` открывай только если кодовый snippet не отвечает на вопрос или затронут boundary.
- Skill загружай, только если его trigger точно совпал с задачей; не загружай skill из-за общего слова вроде “Vue” или “bug”, если правка локальная и очевидная.
- Большие reference-файлы skill'а открывай только по конкретному вопросу или для `FULL_LOOP`.
- Если skill требует читать энциклопедию для маленькой правки, применяй condensed checklist и зафиксируй в финале, что deep refs не нужны из-за класса задачи.

Пример: Vue `.vue` правка в одном компоненте — достаточно Composition API / props-emits / cleanup чеклиста и snippets. Полные Vue references нужны при сложной реактивности, composable API, SSR/router/store или спорной архитектуре.

## 3. Fast paths

### Eden UI / titlebar / sidebar

1. `rtk grep` по компоненту/CSS/test id/window IPC.
2. Читать snippets только в `App.vue`, нужном child component/CSS или preload/host IPC.
3. `docs-site/apps/eden/ui.md` и узкие forbidden открывай, если меняешь визуальные правила, titlebar/safe-area или tokens.
4. Check: `rtk bun run desktop:typecheck`.
5. Visual verify обязателен для layout/цветов/иконок; если не сделан — сказать явно.

### Eden editor / CM / TipTap

1. Читать: `docs-site/apps/eden/editor.md`.
2. Сначала решить, какой path затронут: CodeMirror, TipTap или selection в `App.vue`.
3. Не читать оба редактора целиком без причины; ищи `editable`, `onSave`, `updateListener`, `EditorContent`, scroll/title state.
4. Check: `desktop:typecheck`; для поведения редактора — targeted unit/e2e или visual verify.

### Eden typed objects / person / image

1. Читать: `docs-site/apps/eden/typed-notes.md`.
2. Основные файлы: `TypedHeader.vue`, `ObjectPropertyField.vue`, `ObjectPropertyPicker.vue`, `systemTypes.ts`, `objectImages.ts`.
3. Не трогать ARK/store, если задача только про отображение header fields.
4. Type changes всегда обновляют `type_id`, `header_layout`, `header_props_json` вместе.

### ARK/data/sync/schema

1. Это почти всегда `FULL_LOOP`.
2. Читать: `write-boundary.md`, `ark-objects.md`, `sync.md`, `agents/forbidden/ark.md`.
3. Никаких direct SQL writes в sync tables без version-vector path.
4. Check: `ark:guard:writes`, targeted Rust/TS tests, smoke при substantial.

### Vue в Kosmos

1. Для локальной Vue-правки не открывай все Vue best-practice references.
2. Минимальный чеклист: `<script setup lang="ts">`, props down/events up, computed вместо template logic, cleanup для listeners, scoped/local CSS или tokens.
3. Deep refs only: сложная reactivity/composable/component API/router/store/perf.

## 4. Diff discipline

- В dirty worktree сначала `rtk git status` или `rtk git diff --stat`.
- Не делай широкий `git diff` по repo или большому списку файлов для `LIGHT_LOOP`.
- Для самопроверки используй `rtk git diff -- <один-файл>` или `--stat`; полный diff только перед коммитом/ревью.
- Не смешивай попутный refactor с задачей: один логический change — один diff.

## 5. Verification discipline

Проверка должна доказывать риск изменения, а не быть максимально широкой.

| Изменение                   | Минимальный сигнал                                                     |
| --------------------------- | ---------------------------------------------------------------------- |
| TS/Vue UI                   | `desktop:typecheck`                                                    |
| визуальный layout           | screenshot/Playwright visual verify или честно «не проверял визуально» |
| extension build concern     | `platform/desktop build:extensions`                                    |
| ARK write/data              | `ark:guard:writes` + targeted tests                                    |
| substantial/multi-subsystem | proof loop evidence + smoke                                            |

Noisy output — через `rtk err <cmd>` или лог в `.tmp/*.log`, в чат только ошибки/хвост.

## 6. Stop rules

Остановись и спроси/эскалируй, если:

- `LIGHT_LOOP` требует читать >3 крупных файлов;
- найдено два возможных архитектурных пути и выбор продуктовый;
- проверка невозможна локально, а изменение визуальное/поведенческое;
- задача затронула ARK/data/sync/schema/focus/security/command bus;
- нужно менять generated files вручную вместо source docs.

## 7. Финальный ответ

Коротко:

- классификация (`NO_LOOP`/`LIGHT_LOOP`/`FULL_LOOP`);
- что изменено;
- какие проверки PASS;
- что не проверено;
- если появился reusable lesson — записан shortcut/skill или явно не понадобилось.
