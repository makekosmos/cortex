# Phase 7 — Boot self-check + safeHandle — Evidence

## AC1: boot self-check в main.ts

PASS. `shell/electron/main.ts::runBootSelfCheck()` добавлена и вызывается
первой строкой в `app.whenReady()`. Три проверки:

1. `verifyUserDataMatches(KEPLER_INSTANCE)` — сравнивает кэшированный
   Electron `userData` с резолвнутым из instance.ts. Mismatch → dialog +
   `app.exit(1)`. Функция `verifyUserDataMatches` живёт в `instance.ts`
   (whitelist guard'а `ark:guard:writes` для `app.getPath('userData')`).
2. `existsSync(resolveBackendExe())` — отсутствие → dialog +
   `app.exit(1)`.
3. `KEPLER_INSTANCE.kind === "test" && !process.env.KOSMOS_TEST_MODE` →
   dialog + `app.exit(1)`.

Success path логируется через `keplerLog.info("boot", "self-check passed", ...)`.

## AC2: ipc-safe.ts существует

PASS. Файл `shell/electron/ipc-safe.ts` создан. Exports `safeHandle<P, R>`.

```ts
export function safeHandle<P extends unknown[], R>(
  channel: string,
  handler: (event: Electron.IpcMainInvokeEvent, ...args: P) => Promise<R> | R,
): void {
  ipcMain.handle(channel, async (event, ...args) => {
    try {
      return await handler(event, ...(args as P));
    } catch (err) {
      keplerLog.error("ipc", `${channel} threw`, {
        err: String(err),
        stack: err instanceof Error ? err.stack : undefined,
      });
      throw err;
    }
  });
}
```

## AC3: 10 IPC handler'ов сконвертированы

PASS. Сконвертированы в `shell/electron/main.ts`:

1. `kepler:crashes:list`
2. `kepler:backend:restart`
3. `kepler:search:query`
4. `kepler:commands:list`
5. `kepler:commands:invoke`
6. `kepler:ark:request`
7. `kepler:export:list`
8. `kepler:export:run`
9. `kepler:export:pickDir`
10. `kepler:objects:listRecent`

Остальные ~20 handler'ов оставлены как infrastructure для будущего sweep'а.

## AC4: typecheck

PASS.

```
$ bun run --cwd shell typecheck
$ tsc --noEmit
(exit 0)
```

## AC5: build:js

PASS. `bun run --cwd shell build:js` — все 5 bundles собираются
(main.js, preload, dashboard, eden, delphi, arrancador, horologion).

## AC6: e2e Eden

PASS. 9/9.

```
  ✓ tests\e2e\eden.spec.ts (9 passed, 52.6s)
```

## AC7: oxlint / oxfmt / ark:guard:writes

PASS.

- `bunx oxlint .` — exit 0 (warnings only, все pre-existing).
- `bunx oxfmt --check shell/electron/{main,instance,ipc-safe}.ts` —
  all matched files use correct format.
- `bun run ark:guard:writes` — `ARK write boundary guard passed.`

## Notes / known issues

- Один pre-existing typecheck error: `packages/ark/src/generated/index.ts`
  ссылается на bindings из `crates/ark-core/rust/bindings/`, которые
  генерируются `cargo test --features ts-rs`. Не commit'ятся, prepare шаг
  для CI / local. Прогнал `cargo test --features ts-rs --manifest-path
crates/ark-core/rust/Cargo.toml --lib` — bindings регенерировались,
  typecheck стал зелёный. Не относится к Phase 7.
- `app.getPath('userData')` инкапсулирован в `verifyUserDataMatches`
  (instance.ts) — guard'у нравится, и интент очевиден из имени.
