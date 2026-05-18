# Focus mode — digital-cave + Horologion merger

**Статус:** SPEC ONLY (без implementation). Roadmap entry для будущей разработки.
**Версия Kepler:** 0.1.15+ (требует focus widget infrastructure из 0.1.15).
**Estimate (implementation):** **gut ~1 week (40h)**, adjusted via 5 reliable anchors **~3-4h** wall-clock с parallel subagents. **Calibration row создаётся при старте implementation, не сейчас.**

---

## Зачем

«Фокус-мод» в Kepler — это **слияние** двух концепций:

- **Horologion (что сейчас существует)** — pomodoro / стопwatch / трекинг задач.
- **digital-cave (что зарезервировано как app, не реализовано)** — блокировка отвлекающих приложений / сайтов.

Идея: разные **типы задач** в Horologion → разные **профили блокировок**. Например:
- Pomodoro «Coding» → блокируется TikTok / Reddit / YouTube
- Pomodoro «Writing» → блокируется Twitter + чаты
- Stopwatch «Research» → разрешено всё (только трекинг)

Один экосистемный feature, не два разных app.

## Архитектурная решимость

`digital-cave` **не становится отдельным extension**. Блокировка живёт в Kepler shell как core feature (требует privilege escalation для hosts file, что non-trivial для extension sandbox). Horologion публикует «текущий профиль» в command bus, shell применяет блокировку.

Альтернатива (отброшена): сделать digital-cave отдельным extension с собственным sidecar бинарником, который требует elevation. Слишком много moving parts, фрагментирует UX.

## Компоненты

### 1. Hosts file manipulation

`C:\Windows\System32\drivers\etc\hosts` — Windows механизм перенаправления DNS. Если в файле:
```
127.0.0.1 tiktok.com
127.0.0.1 www.tiktok.com
```
браузер не сможет резолвить tiktok.com на реальный IP → редирект на 127.0.0.1.

**Privileges:** запись в hosts требует admin / `runas` через UAC.

**Подход:**
- Helper bin (`kepler-block-helper.exe`) в `extraResources` — отдельный signed бинарник с manifest требующим elevation (`<requestedExecutionLevel level="requireAdministrator" />`).
- Shell main process spawn'ит helper через `ShellExecuteEx(...,"runas")` когда нужно изменить hosts.
- UAC prompt появляется один раз при первом включении блокировки.
- Helper читает stdin (JSON команды: `{op:"add"|"remove"|"reset", domains:[...]}`) и atomic'но обновляет hosts (backup в `hosts.kepler-backup` first, replace, verify).

**Альтернатива:** не трогать hosts, а поднять локальный DNS proxy на 127.0.0.1:53 → юзеру в network adapter поменять DNS на 127.0.0.1. **Не делать** — ломает все DNS lookups системы если что-то пойдёт не так.

### 2. Blocklist TXT loader

Юзер может загрузить plain TXT файл со списком доменов (1 на строку, `#` — комментарии):
```
# Social media
tiktok.com
twitter.com
x.com
reddit.com

# Video
youtube.com
twitch.tv

# Игнорируется
# comment line
```

Парсер устойчив:
- ignore пустые строки + `#` комментарии
- strip `https://`, `http://`, `www.`
- normalize в `domain.tld` форму
- автоматически добавлять `www.<domain>` вариант если оригинал без www

Persist в ARK как `blocklist_obj` (новый object_type, см. ARK model):
```json
{
  "id": "blocklist-coding",
  "type": "blocklist_obj",
  "props": {
    "name": "Coding focus",
    "domains": ["tiktok.com", "www.tiktok.com", "reddit.com", ...]
  }
}
```

UI: drag&drop TXT → preview parsed domains → save as named blocklist.

### 3. Block page (local HTTP server)

Когда юзер пытается зайти на `tiktok.com` через hosts redirect → попадает на `127.0.0.1`. Если у него ничего на 127.0.0.1:80 не слушает — браузер показывает «Connection refused», плохой UX.

Решение: Kepler shell поднимает локальный HTTP server на `127.0.0.1:8080` (или unused port). Hosts entries: `127.0.0.1 tiktok.com` + добавить в hosts тоже port forwarding нельзя — hosts file не поддерживает порты.

**Workaround:** использовать `8080` не получится из-за hosts ограничений. Альтернативы:
- (A) Слушать на **порте 80**, но это требует admin privileges + конфликт с любым другим веб-сервером.
- (B) Использовать `127.0.0.1` как IP, но юзер увидит "site cannot be reached" от браузера, а Kepler никак этому не помочь без полноценного proxy.
- (C) Подменять hosts на специальный IP `127.0.0.42` и spawn'ить mini HTTP server на `127.0.0.42:80` — требует admin для bind на 80 порт.
- (D) Использовать reverse proxy с TLS termination — слишком сложно для personal launcher.

**Решение:** Вариант (A) с graceful degradation:
- Helper bin при первой блокировке spawn'ит mini HTTP server на 127.0.0.1:80 (requires admin, делает один раз через тот же elevation flow что и hosts modify).
- Server рендерит статическую HTML страницу «Этот сайт заблокирован Kepler Focus» с кнопками:
  - **«ОК, закрыть»** — закрывает таб (через `window.close()` если возможно, иначе плашка).
  - **«Нужно на 5 минут»** — клик отправляет HTTP POST в Kepler shell (через `127.0.0.1:<known-port>/unblock?domain=tiktok.com&duration=300`), shell делает temporary unblock (удаляет hosts entry на 5 мин, потом снова добавляет).

Mini HTTP server pages bundled in helper bin, не зависят от extension renderer.

### 4. 5-минутный temporary unblock

Когда юзер кликает «Нужно 5 минут»:
1. Shell получает POST от block page.
2. Shell спрашивает helper: «remove `tiktok.com` from hosts, restore after 300s».
3. Helper удаляет entry → schedule re-add через `setTimeout` (или persistent timer если перезапуск).
4. Browser cache flush hint показывается user'у («обнови страницу или Ctrl+F5»).
5. Через 5 минут — re-add entry. Юзер на сайте получает Connection refused / автоматический redirect обратно на block page при следующей навигации.

**Edge cases:**
- Множественные «5 минут» для разных доменов — track per-domain timers.
- Kepler crashes / restart во время unblock — на startup проверяем pending temporary unblocks в персистентном state, восстанавливаем timers.
- Юзер хочет cancel unblock — кнопка в Settings → Focus → «Активные временные разблокировки».

### 5. Horologion integration

В Horologion pomodoroDraft расширяется полем `focusProfileId: string | null` — id блок-листа, который будет применён на старте pomodoro.

```typescript
pomodoroDraft = {
  title: "Implement focus mode",
  tasks: [...],
  focusProfileId: "blocklist-coding", // или null если без блокировки
}
```

При `pomodoro.start`:
- Horologion публикует команду `focus:enable` через command bus с payload `{profileId: "blocklist-coding"}`.
- Shell main listens → applies blocklist через helper.

При `pomodoro.stop` / `pomodoro.finish` / `pomodoro.pause` (опция: блокировка на паузе off):
- Horologion публикует `focus:disable`.
- Shell main → helper → restore hosts.

Если pomodoro прервался crash'ем — shell на startup чекает `was_blocking_active` flag в state, спрашивает юзера «Восстановить блокировку TikTok / Twitter? Pomodoro был прерван».

### 6. UI

**Kepler Settings → Focus tab** (новый):

- **Blocklists**: список созданных, кнопка «Создать» (drag&drop TXT или paste plain text).
- **Active blocks**: что сейчас заблокировано (если есть). Кнопка «Снять блокировку» (только если pomodoro не active — иначе блокировка завязана на pomodoro state).
- **Активные временные разблокировки**: домен + countdown «через 3:42 снова заблокируется».
- **Helper status**: «Helper installed: yes/no», «Last elevation: 2026-05-18 14:23», кнопка «Переустановить helper» (re-elevation).
- **History**: какие сессии запускались с каким blocklist, общая длительность блокировок за день / неделю.

**Horologion Settings → Focus profiles** (extension settings):
- В PomodoroDraftInput → новый dropdown «Focus profile» → выбор blocklist из созданных в Kepler Settings.

**Focus widget enhancements (uses 0.1.15 widget):**
- Если активный pomodoro имеет focusProfileId → виджет показывает иконку 🛡️ рядом с временем как индикатор «блокировка active».
- Click на иконку 🛡️ → open Settings → Focus tab.

## Acceptance Criteria

| # | AC | Verify |
|---|---|---|
| AC1 | Helper bin `kepler-block-helper.exe` собран, подписан, с manifest требующим elevation | electron-builder afterPack copy + signtool verify |
| AC2 | Первое включение блокировки показывает UAC prompt; subsequent calls используют cached elevation (если supported) | manual smoke |
| AC3 | TXT blocklist parser корректно обрабатывает: пустые строки, # комментарии, http:// prefix, www. prefix, mixed case | unit tests на helper / parser module |
| AC4 | Atomic hosts update: backup → write → verify → rollback на любом failure | unit test против fake hosts file |
| AC5 | Mini HTTP server на 127.0.0.1:80 рендерит block page с двумя кнопками; closes на «ОК», POST на «5 минут» | manual smoke + curl |
| AC6 | 5-минутный temporary unblock: hosts entry removed → 300s timer → entry restored. Persist через restart Kepler. | unit test + manual smoke с restart mid-timer |
| AC7 | Horologion `pomodoro.start` с focusProfileId публикует `focus:enable` command в bus; shell listens и применяет блокировку | e2e или manual smoke |
| AC8 | Kepler Settings → Focus tab показывает blocklists, active blocks, temporary unblocks, helper status | manual smoke |
| AC9 | Uninstall Kepler → hosts file ВСЕГДА восстанавливается из backup (не оставлять блокировки) | uninstall script + manual verify |
| AC10 | Browser cache hint показывается юзеру когда temporary unblock activated (DNS cache flush instruction) | UI text in block page |

## Что НЕ входит в scope

- ❌ Process-level блокировка приложений (TikTok desktop app, не браузер). Это другой механизм (Windows AppLocker / process monitor) — отдельный сценарий, отложенный для phase 2.
- ❌ Cross-platform support. Kepler — Windows-only сейчас. macOS/Linux hosts file path другой, helper нужен другой; отложено.
- ❌ Регулярные выражения / wildcards в blocklist. Только exact domain match + автоматический www.subdomain.
- ❌ Cloud-sync blocklists между устройствами юзера. Local-only.
- ❌ "Inverse" mode (allowlist вместо blocklist — разрешить только X). Может позже.
- ❌ Browser extension которая помогает (DNS cache flush автоматический). Только hosts approach.
- ❌ Notifications «вы хотели зайти на tiktok.com 5 раз за час — может стоит выключить telegram tag?». Analytics — phase 2+.

## Риски

- **Helper bin code-signing:** без подписи Windows SmartScreen / Defender может flag'нуть. Решение: signtool с self-signed cert (warning остаётся) ИЛИ купить EV cert (~$200/год).
- **UAC fatigue:** если elevation требуется на каждый toggle блокировки → юзер устанет. Решение: helper остаётся резидентным после первого elevation (long-running process), shell общается через named pipe / TCP localhost.
- **Hosts file conflicts с другими блокировщиками:** если у юзера уже стоит uBlock / hosts-based adblock (типа Pi-hole редиректит). Marker `# kepler-focus BEGIN` / `# kepler-focus END` в hosts — модифицируем только между ними.
- **Recovery если Kepler crashes mid-block:** persistent state + reconciliation on startup.
- **Privacy:** hosts file видят все программы на машине. Юзеру **не нужно** знать что Kepler пишет туда специальные entries — но они там видны. Не privacy-critical, но note.

## Зависимости

- 0.1.15 Focus widget (✅ done) — UI индикатор активной блокировки.
- ARK schema: новый `blocklist_obj` object_type (additive schema migration, без destructive).
- Helper bin build pipeline (Rust? C++? Go?) — выбор языка. Rust подходит (есть toolchain уже в repo).

## Roadmap phases

| Phase | Scope | Estimate (adjusted) |
|---|---|---|
| **F1** | Helper bin + hosts modification + atomic backup/restore | 1h |
| **F2** | Blocklist TXT loader + ARK object_type + Settings UI | 0.5h |
| **F3** | Mini HTTP server на 127.0.0.1:80 + block page | 0.7h |
| **F4** | 5-min temporary unblock + persistent state | 0.5h |
| **F5** | Horologion integration (focusProfileId + command bus) | 0.3h |
| **F6** | Focus widget enhancement (🛡️ icon) + Settings Focus tab UI | 0.4h |
| **F7** | E2E smoke tests + uninstall hosts restore | 0.6h |

**Cumulative adjusted estimate: ~4h wall-clock** (с anchors variance).

## Не приступать пока

- Storybook setup (другая текущая задача) — completed.
- HandCrafted tag convention — completed.
- Delphi task creation redesign — completed.

После этого — приступать к **F1** в отдельном proof loop'е `.agent/tasks/<DATE>-focus-mode-F1-helper/`.
