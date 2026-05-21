# Тестирование — правила для агента

::: tip Сначала прочитай
[Изоляция тестовых БД](/concepts/test-isolation) — жёсткое правило про `KOSMOS_DATA_DIR` и `tests/.e2e/<spec>/`. Эта страница — про **как писать e2e**, что считать стандартным паттерном, и какие ловушки уже известны.
:::

## Уровни тестов

| Уровень | Где | Что | Когда писать |
|---|---|---|---|
| Rust unit | `crates/ark-core/rust`, `services/kepler-backend` | `cargo test` — ARK runtime, lock-file, WS handshake, sync | Новый Rust код / refactor |
| TS unit | `packages/ark/tests/` | `bun test` — `@kosmos/ark` SDK contracts | Новый SDK метод / lock-file resolver |
| **Extension contract (universal)** | `tests/e2e/extensions-contract.spec.ts` | Manifest-driven: boot + commands.register + ARK round-trip | **Автоматически** для каждого extension с `manifest.tests` |
| **Per-app UI spec** | `tests/e2e/<app>.spec.ts` | Конкретный UI flow (TipTap render, Pomodoro tick) | Когда фича не покрывается архитектурным contract'ом |

## Universal extension contract (manifest-driven)

Каждый Vue extension в манифесте объявляет минимальный test contract:

```jsonc
{
  "id": "eden",
  // ...
  "tests": {
    "commands": ["eden:note:create", "eden:note:search"],
    "smoke": {
      "objectType": "note_obj",
      "sample": { "title": "contract-smoke-eden" }
    }
  }
}
```

`tests/e2e/extensions-contract.spec.ts` дискаверит все `extensions/*/manifest.json`, для каждого с `tests` блоком генерирует один test:

1. Открыть через `kepler.commands.invoke("<id>:open")`
2. Дождаться extension window (timeout 10s)
3. Проверить `commands.list` содержит все объявленные `tests.commands`
4. ARK round-trip: `upsert_object` sample → `get_object` → `delete_object`

**Когда добавлять `tests`** в новый extension: всегда. Это архитектурный baseline — extension должен загружаться и базово взаимодействовать с ARK.

**Когда расширять per-app spec** в дополнение: когда у фичи есть нетривиальный UI flow (editor, timer, list operations с side-effects).

## Headless mode

::: danger ВСЕГДА. Без исключений.
**Любой e2e прогон обязан быть невидимым для пользователя.** Не должно мелькать ни одно окно — ни launcher, ни extension, ни Settings, ни Dashboard, ни focus widget, ни install dialog. Если ты добавил новое BrowserWindow / show() / showInactive() / setAlwaysOnTop() — обязан проверить, что оно не появляется при `KOSMOS_HEADLESS=1` или `KOSMOS_TEST_MODE=1`. Пользователь не должен видеть никакой мигающей UI-активности от тестов.
:::

Все e2e тесты идут с env `KOSMOS_HEADLESS=1` + `KOSMOS_TEST_MODE=1` (выставляет `launchKepler` automatically). Все BrowserWindow респектят оба флага — `show: false`, `skipTaskbar: true`, `showLauncher()`/`showInactive()` skip'ают визуальное всплытие, `setAlwaysOnTop` подавляется.

Это **обязательное** правило: в e2e никогда не показывать windows. Иначе тесты воруют focus у пользователя, мешают работе и flake'ят на slow paint.

Канонический pattern для нового BrowserWindow:

```ts
const headless =
  process.env.KOSMOS_HEADLESS === "1" || process.env.KOSMOS_TEST_MODE === "1";
new BrowserWindow({
  show: !headless,
  skipTaskbar: headless,
  // ...
});
```

Для `.show()` / `.showInactive()` / `.focus()` / `.setAlwaysOnTop(true)`, вызываемых **после** create:

```ts
if (process.env.KOSMOS_HEADLESS === "1" || process.env.KOSMOS_TEST_MODE === "1") {
  return; // или skip только визуальные операции, оставив state / IPC
}
win.showInactive();
```

Главное shell-окно (launcher) и так стартует с `show: false` — но `showLauncher()` (вызывается из hotkey, tray-click, post-update flow) обязан проверять headless перед `mainWindow.showInactive()`. См. `shell/electron/main.ts::showLauncher()`.

Запрет CLAUDE.md дублирует это правило в секции «Тесты». Любое нарушение — fail из-за visible window — это **баг**, не «фича тестов».

## Известные ловушки в e2e (и как их обходить)

### 1. Command bus warmup race

**Симптом:** `app.waitForEvent("window", { timeout: 10_000 })` timeout после `commands.invoke("<id>:open")`.

**Причина:** `window.kepler.commands.invoke` ещё не exposed preload'ом или backend WS handshake ещё не завершён. `invoke()` тихо no-op'ит.

**Решение:** перед invoke — backend warmup и polling:

```ts
await launcher.waitForLoadState("domcontentloaded");
await launcher.waitForTimeout(2500);  // backend WS + ArkClient warmup

const triggered = await app.evaluate(async ({ BrowserWindow }) => {
  const launcher = BrowserWindow.getAllWindows()[0];
  const start = Date.now();
  while (Date.now() - start < 3000) {
    const ok = await launcher.webContents.executeJavaScript(`
      (async () => {
        if (typeof window.kepler?.commands?.invoke !== "function") return "no-api";
        try {
          await window.kepler.commands.invoke("<id>:open");
          return "ok";
        } catch (e) { return "throw:" + e.message; }
      })()
    `);
    if (ok === "ok") return "ok";
    await new Promise(r => setTimeout(r, 100));
  }
  return "timeout";
});
expect(triggered).toBe("ok");
```

См. `tests/e2e/extension-ark-bridge.spec.ts` и `tests/e2e/eden.spec.ts` (helper `openEden`).

### 2. Vue `<transition>` оставляет обе view'хи в DOM

**Симптом:** `getByRole("button", { name: /Начать сессию/ })` strict-mode fails — 2 элемента.

**Причина:** между Pomodoro ↔ Stopwatch есть Vue transition, во время анимации оба компонента в DOM. Тест locator'ит до завершения анимации.

**Решение:** ждать пока старый элемент detached:

```ts
await horoWindow.getByRole("tab", { name: "Секундомер" }).click();
await horoWindow.waitForSelector(".pomo__primary", { state: "detached", timeout: 3_000 });

// Теперь только .sw__primary в DOM:
const startBtn = horoWindow.locator(".sw__primary");
await expect(startBtn).toBeVisible();
```

Альтернатива — scope locator к специфичному классу (`.sw__primary`, `.pomo__primary`) вместо общего `getByRole`.

### 3. Default route extension'а ≠ inbox

**Симптом:** «Delphi не показывает задачу после reopen». Body = «На сегодня задач нет».

**Причина:** `delphi:open` deep-link'ает в `/today`. Задача создана с `isToday: false` → она в Inbox (`/`), не в Today.

**Решение:** явно навигировать в Inbox перед assertion:

```ts
await delphi.getByText("Входящие", { exact: true }).first().click();
await delphi.waitForTimeout(800);
```

Или: создавать задачу с правильными флагами (`isToday: true` если тест про Today).

### 4. Lazy object_type registration → FK constraint

**Симптом:** universal contract spec падает с `FOREIGN KEY constraint failed` на `upsert_object`.

**Причина:** Extension регистрирует свой object_type лениво (на первом write). Contract spec пытается upsert до того, как extension сам это сделал.

**Решение:** в shim extension'а сделать registration **eager** на module init:

```ts
// extensions/<id>/src/lib/<shim>.ts
const initialArk = kepler();
if (initialArk) {
  void ensureMyObjectTypeRegistered(initialArk).catch(() => {});
}
```

Не lazy при первом upsert. См. `extensions/delphi/src/lib/electron-api-shim.ts` (Phase 6.0.5 fix).

### 5. Stale ACL lock-files (Windows)

**Симптом:** `EPERM` в `fs.rmSync(tests/.e2e/<slug>/)`.

**Причина:** `kepler-backend.exe` создаёт `kepler.lock.json` через `icacls /inheritance:r /grant:r %USERNAME%:F` (security: lock содержит auth token). Если username от прошлого test run отличается (другой Windows user account) — текущий user не может прочитать/удалить файл.

**Решение (штатное):** backend поддерживает env-флаг `KOSMOS_LOCK_PERMISSIONS_DISABLED=1` — при нём `apply_owner_only_permissions` пропускает `icacls` (Win) / `chmod 0600` (Unix). `launchKepler` helper выставляет его автоматически рядом с `KOSMOS_HEADLESS=1`, поэтому новые `tests/.e2e/<slug>/` lock-файлы не имеют жёсткого ACL и удаляются `freshDataDir` без проблем.

::: warning Prod НИКОГДА не выставляет этот флаг
Lock содержит auth token к ARK DB. Без ACL он читаем любым процессом текущей машины. Флаг — строго test-only, ставится только из `tests/e2e/helpers/launch.ts`.
:::

**Одноразовый cleanup для уже накопленных stale lock-файлов** (требует admin, нужен один раз после первого pull этого fix'а):

```powershell
Get-ChildItem tests\.e2e -Recurse -Force -Filter kepler.lock.json | ForEach-Object {
  takeown /F $_.FullName /A
  icacls $_.FullName /reset /T
  Remove-Item $_.FullName -Force
}
```

## Чек-лист: я написал/правил e2e spec

- [ ] `launchKepler({ slug: "<unique>" })` — slug не пересекается с другими spec'ами.
- [ ] Backend warmup перед `commands.invoke` (2.5s после `waitForLoadState`).
- [ ] Если test не tolerant к warm-up race (как `extension-ark-bridge`) — polling-pattern до 3s.
- [ ] Locator'ы scope'ятся к специфичным CSS классам, не общим role-based, если на странице потенциально несколько подходящих элементов (Vue transitions, multiple panes).
- [ ] Headless-flag не overridden в `opts.env` (если опечатался — `KOSMOS_HEADLESS=1` остаётся, окна не лезут).
- [ ] `app.close()` или `app.quit()` в `finally` — кепнул процесс.
- [ ] Test name на русском («horologion: ...» — convention).
- [ ] Если spec тестирует UI flow специфичный для extension — добавлен `tests` блок в manifest.json для universal contract coverage.

## Чек-лист: я добавил новый extension

- [ ] `manifest.json` имеет поле `tests`:
  ```json
  "tests": {
    "commands": ["<id>:command1", "<id>:command2"],
    "smoke": { "objectType": "<type_id>", "sample": { "title": "contract-smoke-<id>" } }
  }
  ```
- [ ] Если extension использует custom object_type, который не зарегистрирован в `crates/ark-core` builtin types — eager registration в shim на boot (см. ловушку #4).
- [ ] Per-app UI spec `tests/e2e/<id>.spec.ts` для нетривиальных flow'ов (helper `open<Id>(app)` для повторного использования).

## Связанные документы

- [Proof loop](/concepts/proof-loop) — структура `.agent/tasks/<DATE>-<slug>/` для substantial задач.
- [Изоляция тестовых БД](/concepts/test-isolation) — `KOSMOS_DATA_DIR` контракт.
- [Estimation](./estimation) — оценка времени на задачу + калибровка.
- [Extension host](/concepts/extension-host) — manifest spec, openExtension, headless behavior.
