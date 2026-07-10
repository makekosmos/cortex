# Eden bubble diary: ARK storage, date labels, and one-level threads

Status: **FROZEN before implementation**  
Task class: **FULL_LOOP** (ARK data/write boundary, migration, links, and UI behavior)

## Original task

> На странице дневника в Eden bubble всегда показывает только время, поэтому на следующий день старая запись выглядит как сегодняшняя. Нужно корректно различать сегодня, вчера и более старые даты. В `sample/Pile-main` есть пример связанных записей/тредов — нужно грамотно реализовать такие связи для bubbles. Сейчас bubbles, похоже, живут только в `localStorage`; они должны храниться в ARK как дневниковые объекты.

## Goal

Make ARK the only canonical store for Eden diary bubbles, preserve existing local data through a safe migration, show unambiguous local-date labels, and add durable one-level reply threads using existing ARK object/link APIs.

## Frozen decisions

1. A bubble is an ARK object with `typeId = "system-type-journal"` and `propsJson.entry_kind = "bubble"`. Do not add a bubble object type.
2. Bubble body uses Eden's existing entry content codec. Bubble-specific metadata stays in `propsJson`: `bubble_kind` (`plain | idea | task | highlight`) and `tags` (`string[]`). `createdAt` is the occurrence timestamp used for ordering and date/time display; edits preserve it and advance `updatedAt`.
3. The existing `eden-bubble-diary-local-bubbles` key is migration input only. After migration starts there is no ongoing localStorage write and no ARK/localStorage dual-write.
4. A reply is a separate bubble object. Its durable edge is an ARK `object_links` record from child to parent with `linkType = "reply_to"`.
5. The UI supports exactly one nesting level, matching the useful interaction from `sample/Pile-main`: roots can receive replies; replies cannot receive replies. The sample is a visual/interaction reference only; its file storage, dependencies, and code are not copied.
6. Deleting a root preserves every reply: its incoming `reply_to` links are explicitly removed/detached and the former children render as roots. Deleting a child removes only that child (and its own outgoing reply link).
7. Existing `list_objects_by_type`, `get_object`, `upsert_object`, `delete_object`, `list_object_links`, `upsert_object_link`, and `delete_object_link` operations are sufficient. No ARK endpoint, table, object type, or schema migration is allowed unless implementation evidence first demonstrates that these APIs cannot meet a frozen AC.

## In scope

- ARK-backed bubble load/create/edit/kind/tags/delete behavior through the Eden API/shim boundary.
- Filtering `system-type-journal` objects by `propsJson.entry_kind === "bubble"` for the bubble timeline.
- Excluding bubble objects from the ordinary notes/journal-entry lists and excluding ordinary journal/note objects from the bubble timeline.
- Safe migration of valid records from the existing localStorage envelope, plus preservation of the current narrowly recognized dated-journal migration path without deleting a source before its bubbles are durable.
- Durable one-level replies and Pile-inspired root/reply rail/connector presentation using existing Kosmos visual tokens.
- Live refresh through the current object/entity event flow to the extent it already exposes object and link changes.
- Focused model, component, migration, and ARK integration coverage; proof-loop evidence and visual evidence.

## Out of scope / non-goals

- Arbitrarily deep/nested threads, reply-to-reply UI, thread collapse, reparenting, drag-and-drop, reactions, mentions, AI reflection, or search/index changes.
- A new bubble object type, ARK table, RPC endpoint, sync protocol, event type, background service, dependency, or generic repository abstraction.
- Copying Pile's file/frontmatter model or changing the ordinary full-page daily journal workflow.
- Retrofitting bubbles into the general note editor, note sidebar, collections, or exporting them as ordinary notes.
- Guessing dates for incomplete legacy records or silently dropping unresolved migration input.

## Required behavior and acceptance criteria

### ARK source of truth

**AC1. Canonical record contract and separation.** A newly created bubble is durably readable as an ARK object with `typeId = "system-type-journal"`, `propsJson.entry_kind = "bubble"`, supported `bubble_kind`, normalized unique string tags, existing Eden body encoding, valid `createdAt`/`updatedAt`, and `deletedAt = null`. Bubble loading selects only non-deleted objects with that exact marker. An ordinary `system-type-journal` or `note_obj` object without the marker never appears as a bubble, and a marked bubble never appears in Eden's ordinary note/journal list or collection.

**AC2. Load and ordering.** Mount/reload obtains bubbles from ARK, not localStorage, and orders roots newest-first by occurrence time. Replies are ordered deterministically within their root (oldest-first, then stable ID tie-break is acceptable). A failed/partial read does not replace already rendered valid data with fabricated local data.

**AC3. CRUD and metadata.** Creating, editing body text, changing parsed hashtag tags, and changing kind each persist through the existing Eden/ARK write boundary and survive a full remount/read-back. Editing preserves object ID and `createdAt`, updates `updatedAt`, and does not erase unrelated object properties. Deleting uses the existing ARK delete path/tombstone behavior; there is no direct SQLite write.

### Unambiguous time labels

**AC4. Local date labels.** Labels are derived from the bubble occurrence timestamp in the user's local timezone and use these Russian forms:

- same local calendar day: `HH:mm`;
- previous local calendar day: `Вчера, HH:mm`;
- older date in the current year: `d MMM, HH:mm`;
- date in another year: `d MMM yyyy, HH:mm`.

The formatter compares local calendar dates, not elapsed 24-hour durations. Its tests cover today, yesterday across a month/year boundary, older same-year, older prior-year, and timestamps around local midnight.

**AC5. Midnight/focus refresh.** A mounted diary recomputes visible labels when the local day rolls over, without remounting or writing the bubbles. It also recomputes on window focus/visibility restoration so a throttled/suspended midnight timer cannot leave stale labels. A bubble shown as `HH:mm` before midnight becomes `Вчера, HH:mm` after the local-day change.

### One-level reply threads

**AC6. Durable reply creation and reload.** Replying to a root creates a separate canonical bubble and then a child-to-parent `reply_to` link. After a full application/remount reload, ARK objects plus links reconstruct the same root/reply grouping. If link creation fails after the child write, the child remains recoverable as a root; it is not discarded. Duplicate submission/retry does not create duplicate links.

**AC7. Exactly one displayed level.** Only roots expose the reply action/composer. Replies render once beneath the root with a clear rail/connector relationship inspired by `sample/Pile-main`, while retaining Eden controls and accessibility. A malformed link (missing/deleted target, self-link, cycle, or child targeting another child) never hides a bubble or produces deeper nesting: the affected bubble is displayed as a root and the invalid relationship is ignored/detached through existing link APIs.

**AC8. Delete semantics.** Before deleting a root, all `reply_to` links targeting it are explicitly deleted; read-back/reload shows its former replies as independent roots with their content and metadata intact. Deleting a child deletes only that child and its outgoing `reply_to` link; the root and sibling replies remain. Focused tests cover both cases and reload from ARK after each operation.

### Migration and sync-aware refresh

**AC9. Idempotent localStorage migration.** Valid versioned records from `eden-bubble-diary-local-bubbles` are mapped to deterministic, collision-resistant ARK IDs and preserve body/rich content where supported, tags, kind, and the best source-backed absolute occurrence timestamp. Re-running migration after interruption upserts/recognizes the same objects and creates no duplicates. The view stops writing the source key as soon as ARK persistence is active.

**AC10. No invented legacy dates; cleanup only after proof.** A legacy record may be migrated only when an absolute date/instant is recoverable from trustworthy source data (for example a valid explicit date plus time, finite stored sort timestamp, stable timestamp-bearing legacy ID, or the recognized dated-journal source metadata). A time-only or otherwise ambiguous record is not assigned today's date: it stays preserved in migration source, is not shown as today's ARK bubble, and leaves migration incomplete/diagnosable. Each migrated object must be read back from ARK and match required content/metadata before its source is removed or a completion marker is written. Source cleanup never precedes successful durable write plus read-back. Recognized legacy dated-journal source objects likewise are not deleted until every derived bubble passes read-back.

**AC11. Current event-flow refresh.** Mounted Eden refreshes affected bubble data on existing `object_upserted` / `object_deleted` events. If the current `entity_changed` payload exposes `object_link` changes, it also refreshes thread grouping on those existing events. Evidence must state which current events were observed. No polling or new ARK/event endpoint is added solely for this feature; if the runtime does not currently expose link changes, object changes refresh live and link grouping is guaranteed on explicit reload.

### Boundary and proof

**AC12. Existing boundary only.** Renderer/components call the Eden API/shim; they do not call `window.kepler.ark.request` directly and never open SQLite. All target writes go through existing ARK RPC/SDK operations and preserve normal sync versioning/tombstones. `bun run ark:guard:writes` and `bun run ark:smoke` pass. No new table/schema/type/endpoint/dependency is present unless a frozen-spec exception is recorded as a separate follow-up task rather than silently broadening this one.

**AC13. Tests and visual evidence.** Fresh focused checks cover AC1-AC11, including unit tests for label boundaries/migration/tree normalization, browser component tests for create/edit/tags/kind/delete/reply UI, and an isolated ARK smoke proving reload and durable links. A headless Eden smoke captures screenshots showing (a) today/yesterday/older labels together and (b) one root with replies and connector treatment. Evidence records the commands, artifacts, and a per-AC `PASS`/`FAIL`; visual work is not declared PASS without the screenshots.

## Migration rules

1. Read the existing localStorage envelope once as source data; never normalize an unknown date to `new Date()`/today.
2. Classify every record as migratable, already migrated, or unresolved. Use a deterministic namespaced target ID so retry is safe.
3. Upsert the bubble through existing ARK operations, then read it back and compare ID, marker, body, tags, kind, and occurrence time.
4. For a reply migration (if a future/source record already carries a parent), write/read the child first, then write/read its `reply_to` link. Current local format does not require inventing parent relationships.
5. Remove only verified source records. Remove the whole key or write a completion marker only when no unresolved records remain. Otherwise retain unresolved source records and emit a diagnosable warning/state.
6. Never delete a recognized legacy dated-journal source until all bubbles derived from it are verified in ARK.

## Implementation constraints

- Reuse the existing Eden entry mappers/content codec, system journal type, ARK operations, subscription bridge, and visual tokens.
- Keep bubble-specific filtering in shared data/model logic so initial load and live-refresh use the same predicate.
- Preserve rich content on migration/read. Existing plain-text edit behavior may remain; this task does not require a new rich editor.
- Use stable IDs for objects and links; a reply link ID must be deterministic for `(childId, parentId, "reply_to")`.
- Register all event/timer/listener cleanup on unmount/scope disposal.
- Do not modify generated `dist` output as source and do not perform unrelated refactors.

## Verification plan

Minimum fresh proof after implementation:

1. Run focused Bun model/migration tests, including fixed-clock local-midnight cases.
2. Run `bunx vitest run --browser --config products/eden/vitest.config.ts products/eden/tests/components/BubbleDiaryView.spec.ts` plus any new focused bubble component spec.
3. Run an isolated ARK integration/smoke that creates root + reply + link, restarts/reloads, verifies grouping, deletes root, and verifies detached reply survival.
4. Run `bun run ark:guard:writes`.
5. Run `bun run ark:smoke`.
6. Run the relevant Eden type/build check selected by the builder and record the exact command.
7. Run headless Eden UI smoke and save the two required screenshots under this task's proof artifacts.
8. Write `evidence.md` and `evidence.json`, then have a fresh verifier judge every AC against current code/output. If any AC is not PASS, write `problems.md`, apply the smallest fix, and reverify.

## Assumptions

- `system-type-journal` is already persisted/available through Eden's existing system-type flow.
- ARK object links are syncable through the existing write path; this task does not change their sync model.
- New bubbles always have a trustworthy current timestamp. Only legacy input can be unresolved.
- The exact connector geometry may adapt to Eden's virtualized timeline, but the resulting root/reply relationship must remain visually obvious and keyboard-accessible at one level.
