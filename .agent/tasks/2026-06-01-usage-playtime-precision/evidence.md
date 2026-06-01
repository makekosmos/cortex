# Evidence — usage playtime precision

## AC status

- AC1: PASS — `usage_sessions.runtime_ms` добавлен в schema/types/upsert/load, tracker держит живые process sessions по `(tracked_app_id, pid)` и увеличивает runtime каждый poll tick до завершения процесса.
- AC2: PASS — `foreground_ms` и `idle_ms` продолжают увеличиваться только для foreground session; background runtime не попадает в foreground/idle.
- AC3: PASS — migration additive-only: `ALTER TABLE usage_sessions ADD COLUMN runtime_ms INTEGER NOT NULL DEFAULT 0` и backfill `foreground_ms + idle_ms`; destructive statements не добавлялись.
- AC4: PASS — `get_usage_analytics` top apps/recent/summary возвращают runtime; Dashboard “Суммарное время” использует `runtimeMs`, а `foregroundMs` показан отдельной колонкой “Активно”.
- AC5: PASS — capture/persist/update/start/finalize ошибки логируются и loop продолжает работу; outer `spawn` перезапускает `run()` после fatal startup/runtime failure.
- AC6: PASS — добавлены/обновлены tests для schema migration, analytics runtime, game playtime runtime, sync usage entity payload и tracker accumulation.
- AC7: PASS — все релевантные проверки ниже прошли.

## Verification

- PASS `cargo test --manifest-path crates\ark-core\rust\Cargo.toml usage_analytics_snapshot_includes_summary_and_zero_filled_trend`
- PASS `cargo test --manifest-path crates\ark-core\rust\Cargo.toml usage_game_playtime_summary_matches_bindings_and_range`
- PASS `cargo test --manifest-path crates\ark-core\rust\Cargo.toml test_init_schema_adds_usage_runtime_ms_to_existing_sessions`
- PASS `cargo test --manifest-path crates\ark-core\rust\Cargo.toml usage_entities_sync_between_two_servers`
- PASS `cargo test --manifest-path services\kepler-backend\Cargo.toml usage_tracker --lib`
- PASS `bun run --cwd shell typecheck`
- PASS `bun run --cwd packages\ark typecheck`
- PASS `bun run ark:guard:writes`
- PASS `bun run docs:check`
- PASS `bun run format:check crates/ark-core/rust/src/db.rs crates/ark-core/rust/src/main.rs crates/ark-core/rust/src/schema.rs crates/ark-core/rust/src/types.rs crates/ark-core/rust/tests/sync_round_trip.rs services/kepler-backend/src/usage_tracker/mod.rs services/kepler-backend/src/usage_tracker/windows_capture.rs shell/src/dashboard/UsageTable.vue shell/src/dashboard/store.ts shell/src/dashboard/types.ts packages/ark/src/ark-client.ts docs-site/services/usage-tracker.md docs-site/concepts/ark-objects.md docs-site/agents/postmortems.md`
- PASS `bun run ark:smoke`
