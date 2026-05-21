# Evidence — ts-rs full coverage

## AC1: cargo test --features ts-rs → 26 PASS

```
cargo test --features ts-rs --manifest-path crates/ark-core/rust/Cargo.toml --lib export_bindings
...
test types::export_bindings_project ... ok
test types::export_bindings_arkobject ... ok
test types::export_bindings_dailytrendpoint ... ok
test types::export_bindings_peerrecord ... ok
test types::export_bindings_syncentity ... ok
test types::export_bindings_objectlink ... ok
test types::export_bindings_heading ... ok
test types::export_bindings_area ... ok
test types::export_bindings_objecttype ... ok
test types::export_bindings_recentsessionentry ... ok
test types::export_bindings_hourlyheatmapcell ... ok
test types::export_bindings_tag ... ok
test types::export_bindings_todoitem ... ok
test types::export_bindings_topappentry ... ok
test types::export_bindings_trackedapp ... ok
test types::export_bindings_usageevent ... ok
test types::export_bindings_usagegamedailytotal ... ok
test types::export_bindings_usagegameplaytimeaggregate ... ok
test types::export_bindings_usagegameplaytimebinding ... ok
test types::export_bindings_usagegamerangetotal ... ok
test types::export_bindings_usageprocesscandidate ... ok
test types::export_bindings_usagesession ... ok
test types::export_bindings_usagesummary ... ok
test types::export_bindings_usagegameplaytimesummary ... ok
test types::export_bindings_usageanalyticssnapshot ... ok
test types::export_bindings_loadalldata ... ok

test result: ok. 26 passed; 0 failed; 0 ignored; 0 measured; 162 filtered out
```

PASS.

## AC2: packages/ark typecheck

```
$ bun run --cwd packages/ark typecheck
$ tsc --noEmit
(exit 0)
```

PASS.

## AC3: shell typecheck

```
$ bun run --cwd shell typecheck
$ tsc --noEmit
(exit 0)
```

PASS — никаких регрессий, существующие импорты `@kosmos/ark` продолжают
работать.

## AC4: oxlint

```
$ bunx oxlint .
(warnings only; 0 errors)
```

PASS (warnings pre-existing — kepler-api-shim, extension-host, etc.).

## AC5: oxfmt --check

```
$ bunx oxfmt --check .
Checking formatting...
All matched files use the correct format.
Finished in 10399ms on 1433 files using 12 threads.
```

PASS.

## AC6: generated/ files

```
$ ls packages/ark/src/generated/
Area.ts                           UsageEvent.ts
ArkObject.ts                      UsageGameDailyTotal.ts
DailyTrendPoint.ts                UsageGamePlaytimeAggregate.ts
Heading.ts                        UsageGamePlaytimeBinding.ts
HourlyHeatmapCell.ts              UsageGamePlaytimeSummary.ts
LoadAllData.ts                    UsageGameRangeTotal.ts
ObjectLink.ts                     UsageProcessCandidate.ts
ObjectType.ts                     UsageSession.ts
PeerRecord.ts                     UsageSummary.ts
Project.ts                        index.ts
RecentSessionEntry.ts             SyncEntity.ts
Tag.ts                            TodoItem.ts
TopAppEntry.ts                    TrackedApp.ts
UsageAnalyticsSnapshot.ts
```

26 generated `.ts` + 1 `index.ts`. PASS.

## Summary

- 23 новых типа добавлены к codegen'у (3 уже были в pilot'е).
- `import-esm` feature ts-rs включён для NodeNext-совместимых импортов.
- `packages/ark/src/generated/**` исключён из oxfmt (raw ts-rs output).
- Part 2 (typed `request<Op>`) — отложено.

All AC PASS.
