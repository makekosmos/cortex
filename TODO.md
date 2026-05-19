# TODO

## UX polish (отложенные)

- [ ] **Полоска загрузки снизу при in-app update.** Когда autoUpdater качает новую версию через UI (Settings → Обновление), показывать тонкий progress bar внизу окна Kepler с процентом download'а. Сейчас обновление триггерится из Settings, но визуальной обратной связи о ходе скачивания нет — пользователь не знает, идёт ли загрузка. Источник прогресса: `electron-updater` `download-progress` event (см. `shell/electron/autoupdater-host.ts`). Renderer-side hook через IPC. Делать когда руки дойдут — не блокирует другие фичи.

## AI-first object graph (2026-05-19)

North star: всё есть объект, любые объекты связываются через `object_links`, AI-агенты — first-class consumer. Markdown — рендеринг для людей, не source of truth. См. memory `project_north_star_object_graph.md` и обсуждение от 2026-05-19.

Action items (приоритет сверху вниз):

- [ ] **MCP server поверх `@kosmos/ark`** — обёртка над существующим WS / command bus в `services/kepler-backend`. Эмитит JSON Schema из `object_types.schemaJson` как tool definitions. Подключается одним `claude mcp add` / Cursor / любой LLM-host. Снимает 90% галлюцинаций агента — schema-driven, не угадывание формата.
- [ ] **`content_md` projection в read/write API.** `get_object` опционально возвращает `content_md` (TipTap JSON → markdown через существующий `note_md` converter из Phase 7). `upsert_object` принимает `content_md` и парсит обратно в TipTap JSON. Source of truth остаётся `content_json`, но для агентов и для человеческого чтения наружу торчит markdown. Нужно достроить обратный конвертер markdown → TipTap, если ещё нет (export-only сейчас).
- [ ] **`get_neighborhood(id, depth)` ARK endpoint.** Возвращает subgraph: узлы (объекты) + рёбра (links) в радиусе N от заданного id. Сейчас агенту чтобы понять «что связано с этой заметкой» нужно `list_object_links` → фильтр → `get_objects_by_ids`. Один endpoint = multi-hop reasoning возможен в один tool-call.
- [ ] **Read-only markdown mirror** под `<data_dir>/notes-md/` (и аналогично для других «человекочитаемых» типов: `tasks-md/`?). Регенерируется на save через `note_md` converter. User видит файлы в Explorer, может grep / git commit / открыть в Obsidian для чтения. Контракт: правки в файлах **игнорируются** (или показываются как hint «создать новую заметку из этого файла»). Source of truth — ARK SQLite. Это закрывает психологический запрос на portability без двойного sync'а.

Не делать:
- ❌ Переход на markdown как primary storage. Ломает typed propsJson + schema, ID-stable wikilinks, cross-type graph, единый sync invariant (HLC + version_vector + tombstones). См. [forbidden.md](docs-site/agents/forbidden.md) и memory `project_north_star_object_graph.md`.
- ❌ Block-level granularity (Logseq transclusion) через файлы. Если когда-то понадобится — реализуется как `block_obj` тип внутри object model.

## Legacy Note

The previous contents of this file described an older `packages/ark/` Python/server-era plan. That is no longer the active ARK architecture.

Current ARK runtime documentation:

- [`packages/ark-core/README.md`](./packages/ark-core/README.md)
- Rust runtime: `packages/ark-core/rust`
- Node/Electron SDK: `packages/kosmos-ark` (`@kosmos/ark`)
- Compatibility SDK name: `packages/arksync-node` (`@arksync/node`)
- Canonical desktop sidecar: `ark-core-rpc`

## Current ARK Priorities

- Keep app writes going through `ark-core-rpc` / `@kosmos/ark`.
- Keep direct Rust writers on `ark_core::db` helpers so sync state is updated consistently.
- Delphi tasks migrate automatically at app startup into `task_obj` records in the generic object model.
- Eden notes and custom typed notes are ARK objects (`note_obj` or their custom object type). Heart remains available for editor/vault-specific behavior and one-time migration/import/export work.
- Arrancador game writes, usage backfill, game hydration, usage summaries, and process search should prefer `@kosmos/ark`; read-only SQLite is only a fallback when the ARK runtime is unavailable.
- Dashboard is a read-only ARK inspector/analytics app. It may inspect selected ARK SQLite databases from Electron main, but must never write to ARK tables.
- Keep the smoke matrix in `docs/ARK-SMOKE-MATRIX.md` current and ensure every automated check uses an isolated test DB or temporary app-data path.
- Use `bun run ark:guard:writes` before changing app data services; direct app-service writes to ARK tables are forbidden.
- ARK runtime now has object query, usage process query, and game playtime summary endpoints. If read-only ARK SQL needs to be removed completely, continue replacing fallback paths with dedicated runtime analytics endpoints.
- Delphi legacy DB sidecar is removed; `task_obj` ARK objects are the task source of truth after startup migration.
