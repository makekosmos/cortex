# Системные требования Kosmos / Kepler

::: tip
Это canonical список того, что нужно для запуска и разработки Kosmos. Обновляется когда меняются platform constraints (новый Electron, новые binary, новый OS minimum).
:::

## Для использования (production)

| Категория | Минимум | Рекомендуется |
|---|---|---|
| **OS** | Windows 10 1809 | **Windows 11 22H2+** (Mica backdrop) |
| **Архитектура** | x64 | x64 |
| **Disk space** | ~400 MB (installer 103 MB + unpacked ~370 MB) | 500 MB free |
| **RAM** | 500 MB free | 1+ GB free |
| **CPU** | любой x64 | dual-core+ |
| **GPU** | integrated с DirectX 11 (для Mica/Acrylic) | discrete или integrated D3D11 |
| **Internet** | для install + auto-update + marketplace catalog | для LAN sync extension |
| **.NET / VC++ Runtime** | не требуется (Electron статически линкуется) | — |
| **Permissions** | standard user | admin для install + focus mode |

### Measured baseline (2026-05-18, production build)

Сценарий «launcher + 4 extensions + Dashboard, 5 min idle» (см. [RAM benchmarks](/concepts/ram-benchmarks)):

- **Launcher only:** 235 MB Private / 382 MB Working Set
- **All extensions idle:** 454 MB Private / 906 MB Working Set
- **11 процессов** (1 main + 4 extension renderers + 1 dashboard + GPU + utility + kepler-backend + ark-core-rpc)

### Что НЕ поддерживается

- ❌ **macOS / Linux** — Windows-only currently (`signtool.exe`, hosts file `C:\Windows\System32\drivers\etc\hosts`, NSIS installer, electron-builder `target=nsis`)
- ❌ **Windows ARM64** — extraResources собраны под x64
- ❌ **Windows 10 pre-1809** — нужна ASAR integrity API
- ❌ **Windows 7/8** — Electron 41 dropped support

## Для разработки

| Tool | Версия | Зачем |
|---|---|---|
| **Node.js** | 20+ | Electron 41 toolchain, vite-plugin-electron |
| **Bun** | 1.x | Workspace package manager (`bun.lock`) |
| **Rust** | 1.80+ | Cargo workspace: kepler-backend, ark-core, focus-helper, focus-svc |
| **Git** | любая | clone + commits |
| **PowerShell** | 5.1+ или 7+ | `scripts/*.ps1` (RAM measurement, baseline orchestrator) |

### Опционально

- **Visual Studio Build Tools** — для `signtool.exe` (electron-builder NSIS signing на production build)
- **Playwright Chromium** — для Storybook Vitest browser tests (`bun run --cwd packages/visuals test`)
- **gh CLI** ИЛИ `GH_TOKEN` env var — для `bun run --cwd shell build` (publish step uploads release to `yoso-industries/kepler-releases`)

## Зависимости компонентов

| Subsystem | Bundle | Внешние требования |
|---|---|---|
| **Electron host** | `Kepler.exe` (Chromium 144 + Node 24) | — |
| **Backend** | `kepler-backend.exe` (Rust + tokio + WS server) | — |
| **ARK storage** | `ark-core-rpc.exe` (SQLite WAL + FTS5 in-process) | — |
| **Focus svc** | `kepler-focus-svc.exe` (Windows Service, AutoStart) | **Один UAC при первой активации блокировки** — Kepler auto-install'ит service (с 0.1.21). Дальше — zero UAC через named pipe. |
| **Focus helper** (fallback) | `kepler-focus-helper.exe` (admin elevation manifest) | UAC per toggle — используется только если юзер отклонил auto-install service'а |
| **AutoUpdater** | `electron-updater` | Доступ к `github.com/yoso-industries/kepler-releases` |
| **Extension marketplace** | github raw + releases | Доступ к `github.com/yoso-industries/kosmos-extensions` |

## Размер на диске (после install)

```
%LOCALAPPDATA%\Programs\Kepler\          ~370 MB
  ├─ Kepler.exe                          ~225 MB (Electron runtime)
  ├─ resources\app.asar                  ~40 MB  (shell + extensions bundles)
  ├─ resources\kepler-backend.exe        ~8 MB
  ├─ resources\ark-core-rpc.exe          ~6 MB
  ├─ resources\kepler-focus-helper.exe   ~1 MB
  ├─ resources\kepler-focus-svc.exe      ~1 MB
  ├─ resources\locales\                  ~1.6 MB (только en-US + ru, см. Exp 08)
  └─ resources\*.pak                     ~80 MB  (Chromium GPU resources)

%APPDATA%\Kosmos\                        растёт по мере использования
  ├─ ark.db                              ~10 MB+ (SQLite, зависит от объектов)
  ├─ extensions\<id>\                    ~10-15 MB per extension
  ├─ extensions-data\<id>\               per-extension settings / cache
  └─ window-state / cache files          ~1 MB
```

## Permissions требуемые runtime

| Действие | Требуется admin |
|---|---|
| Установка через NSIS installer | ✅ (UAC при install) |
| Запуск Kepler launcher / extensions | ❌ |
| ARK operations (read/write objects) | ❌ |
| Установка extension через marketplace | ❌ |
| **Focus mode: первая активация blocklist** | ✅ (один UAC — Kepler auto-install'ит фоновый service) |
| Focus mode: все последующие активации | ❌ (через named pipe, zero UAC) |
| Переустановка / удаление focus service | ✅ (через **Настройки → Фокус → Системный демон**) |
| AutoUpdater download + install | ❌ (per-user install) |

## Известные ограничения

- **Headless / RDP** — globalShortcut Ctrl+Shift+K может не сработать в Remote Desktop session (Windows блокирует hotkey registration в детачнутых session'ах)
- **Multi-monitor** — Kepler launcher позиционируется на primary display; extension окна имеют persisted position per-extension
- **Antivirus / SmartScreen** — `Kepler.exe` и helper binaries **не подписаны EV cert** → Windows SmartScreen warning на первом запуске («Unknown publisher»). Один клик «Run anyway». EV cert ~$200/год — отложено.
- **Group Policy на managed машинах** — может запрещать установку Windows Service (Focus svc), service install fall back на helper-mode (UAC per toggle)

## Целевая аудитория

Kosmos — **personal productivity tool** для одиночного юзера на личной Windows-машине. Не enterprise / не multi-user / не cloud-managed. Если managed корпоративная машина с GPO — некоторые фичи (Focus service, autostart, hosts file modification) могут не работать.

## Related

- [Distribution](/concepts/distribution) — release flow, autoUpdater, marketplace
- [RAM benchmarks](/concepts/ram-benchmarks) — measurement methodology
- [Performance experiments log](/concepts/performance-experiments) — что измеряли и какие результаты
- [Architecture](/concepts/architecture) — общая картина subsystems
