# Spec — Clipboard history вешает мышь (jank) + неограниченная память

Дата: 2026-06-06
Класс: FULL_LOOP (горячий путь main-процесса Electron + производительность + лимиты хранения).

## Проблема (репорт пользователя)

При запущенном Kosmos shell мышь периодически «залипает» и дёргается. При выходе из
приложения — пропадает. Пользователь подозревает clipboard history: ожидал «тихую БД»,
а оно держит огромные данные в памяти.

## Диагноз (по коду)

`shell/electron/clipboard-history.ts` поллит буфер каждые `CLIPBOARD_POLL_MS = 800` мс
в **main-процессе**. В горячем пути три блокирующих операции:

1. **Изображение кодируется каждый тик.** `recordClipboardImage` зовёт
   `image.toDataURL()` (PNG-encode + base64 всего изображения) **до** проверки на
   изменение (`dataUrl === lastSeenImageDataUrl`). Если в буфере висит большой скриншот —
   каждые 800 мс синхронный PNG-encode крупной картинки замораживает event loop.
   → постоянное периодическое подёргивание мыши, даже без копирования.

2. **Синхронный PowerShell на каждое изменение буфера.** `detectClipboardSource` =
   `execFileSync("powershell.exe", … Add-Type … ExtractAssociatedIcon)` с `timeout: 500`.
   Полная синхронная заморозка main-процесса до 0.5 с в момент копирования.

3. **Память и сериализация.** `DEFAULT_MAX_BYTES = 512 MB`, `DEFAULT_MAX_ITEMS = 10_000`,
   нет лимита на размер одной записи. На каждый `record/touch/insert` → `commit()` делает
   `JSON.stringify(items)` дважды + `items.map(withStorageBytes)` (повторный
   `JSON.stringify` по каждому элементу) + синхронный `writeFileSync` всего файла.
   O(n × размер) синхронно на каждое изменение буфера.

## Acceptance criteria

- **AC1 (image):** PNG-encode/`toDataURL` изображения выполняется только при фактическом
  изменении содержимого, а не на каждый тик поллинга. Дешёвый fingerprint (размер +
  сэмплированный хэш raw-битмапа) определяет изменение до тяжёлого кодирования.
- **AC2 (source):** Определение источника (PowerShell) не блокирует main-процесс —
  асинхронный запуск (`execFile`), без `execFileSync` в горячем пути. Перекрывающиеся
  тики не накапливаются.
- **AC3 (limits):** `DEFAULT_MAX_BYTES` снижен с 512 MB до 64 MB. Введён лимит на размер
  одной записи (`maxItemBytes`, дефолт 10 MB): запись, превышающая лимит, не сохраняется.
- **AC4 (persistence):** `commit()` не делает двойной `JSON.stringify(items)` и не
  пересчитывает `storageBytes` по всем элементам каждый раз; сохранение на диск
  асинхронное и дебаунсится.
- **AC5:** `bun run shell:typecheck` зелёный. `bun run ark:guard:writes` зелёный
  (clipboard store вне ARK — не должно регрессить). Юнит-тесты store зелёные.
- **AC6 (honesty):** фактическое исчезновение jank проверяется только пользователем на
  реальном железе — фиксирую как pending manual verify.

## Тесты (RED → GREEN)

`shell/electron/clipboard-history-store.test.ts` (bun:test, чистая логика):

- `defaultClipboardHistorySettings().maxBytes === 64 MB` (AC3).
- `createClipboardHistoryStore({ maxItemBytes })`: `record(hugeText)` → `null`, не в `list()` (AC3).
- `fingerprintImageBytes(w,h,buf)`: одинаковый вход → равный fp; иной → отличается (AC1).

## Вне scope

- Замена поллинга на нативный clipboard sequence number (отдельная задача).
- Перенос source-detection на нативный биндинг вместо PowerShell.
