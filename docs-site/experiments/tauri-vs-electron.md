# Tauri vs Electron — 2026-05-19

::: danger Результат: остаёмся на Electron
На Windows экономия RAM **24%** (не «5-10×» как в маркетинге), на Linux WebKitGTK
ломает TipTap в Eden. Цена миграции 3-6 недель работы. Не оправдано.
:::

## Гипотеза

Аналитики и блоги рекомендуют Tauri как замену Electron с обещаниями «RAM в 5-10
раз меньше, installer в 30 раз меньше, нативный feel». Подозрение: эти цифры —
hello-world vs полное приложение, и на Windows WebView2 — тот же Chromium, RAM
будет таким же. Проверим **прямым замером** на собственной кодовой базе.

## Методология

- Платформа: Windows 11 Pro 10.0.26200 x64.
- Baseline: установленный Kepler 0.2.0 (Electron), измеряем все процессы
  дерева через `Get-Process`/`Get-CimInstance` — `WorkingSet64` (RSS) и
  `PrivateMemorySize64` (private bytes).
- Tauri PoC: Tauri 2.11.2 hello-world (1 окно, без extension'ов, без backend).
- Substantive Tauri port: реальный код (9 IPC handlers, WS proxy к
  kepler-backend, 2 окна, preload-shim над `window.kepler.*`).
- Все замеры воспроизводимы — команды и точные пути в артефактах ниже.

## Замеры

### Disk

| | Electron Kepler 0.2.0 | Tauri PoC | Tauri port |
|---|---|---|---|
| Главный exe | **213 MB** | **3.07 MB** | **4.53 MB** |
| Installer (NSIS) | 113 MB | ~5-10 MB ожид. | ~5-10 MB ожид. |
| Unpacked dist | 385 MB | ~30 MB | ~30 MB + frontend |
| Build cache | ~500 MB (node_modules) | 1.1 GB (Rust target/) | 1.2 GB (Rust target/) |

### RAM idle (4 секунды после cold start, через process tree от parent PID)

| Процесс | Electron Kepler (полное приложение) | Tauri PoC (hello-world) |
|---|---|---|
| Main / host | **157.4 MB** (Node + наш TS) | **22.6 MB** (Rust + tao) |
| Renderer | 117.1 MB (Chromium) | 56.7 MB (WebView2, пустая страница) |
| GPU | 95.9 MB | 69.4 MB |
| Utility | 62.2 MB | 70.3 MB (network + storage) |
| Crashpad | 33.1 MB (наш) | 13.4 MB (WebView2 own) |
| **TOTAL** | **465.7 MB** | **354.8 MB** |

**Tauri host (Rust): 22.6 MB. WebView2 children: 332.2 MB.** То есть пустая
Tauri-страница уже жрёт **76% RAM полнофункционального Electron Kepler** —
потому что WebView2 это тот же Chromium, его process tree почти не отличается.

### Cold start

| | Время до visible window |
|---|---|
| Tauri PoC | **1574 ms** |
| Electron Kepler | ~2-3 с (точно не замерить — launcher hidden by design) |

### Build time

| | cold | incremental |
|---|---|---|
| Electron (`bun run build`) | ~1-2 мин | <30 с (Vite HMR) |
| Tauri PoC (release LTO) | ~1.5 мин | **1.5 мин** (LTO заново) |
| Tauri port (release LTO) | ~1.5 мин (incremental, deps кэшированы) | 1.5 мин |

LTO+strip+opt-level=s даёт маленький exe, но платит этим в incremental build time.
Без LTO Rust incremental ~30-40 c, но exe вырастет до 6-8 MB.

### WebView2 — скрытая стоимость

На той же машине **прямо сейчас**:

| Что | Размер |
|---|---|
| WebView2 runtime на диске (`C:\Program Files (x86)\Microsoft\EdgeWebView\...`) | **829.7 MB** |
| Сейчас запущенные процессы WebView2 от других приложений | **20 процессов, 872.3 MB WS** |

Disk shared между приложениями. RAM — нет, каждое приложение поднимает свои
WebView2 children. Аргумент «Tauri экономит диск» = правда **только если у
пользователя нет других WebView2-приложений** (а в современной системе они есть
почти всегда — Microsoft Teams, Outlook, новый Office).

## Что подтвердилось / не подтвердилось

### Подтвердилось

- **WebView2 = Chromium → RAM на renderer стороне сопоставим с Electron**.
  Сами разработчики Tauri это признают:
  [tauri-apps#5889](https://github.com/tauri-apps/tauri/issues/5889).
- **Linux WebKitGTK ломает наш стек**:
  - TipTap (Eden) contentEditable требует ПКМ:
    [#12638](https://github.com/tauri-apps/tauri/issues/12638),
    [#9088](https://github.com/orgs/tauri-apps/discussions/9088).
  - Font-weight рендерится +100: [#14286](https://github.com/tauri-apps/tauri/issues/14286).
  - Нет WebRTC, WebGPU; custom protocols не отдают audio/video.
  - CSS-анимации блюрят остальной UI.
- **Disk-экономия на главном бинаре реальна**: 3-4 MB vs 213 MB (98% меньше).
  Но build cache Rust вдвое больше node_modules.

### Не подтвердилось

- **«RAM в 5-10 раз меньше»** — реально **−24%** на пустышке. При feature-parity
  будет ещё меньше (renderer вырастет с Vue + TipTap).
- **«Tauri быстрее билдится из-за маленьких артефактов»** — incremental rebuild
  release с LTO 1.5 мин против <30 с у Vite. Это **в 3 раза медленнее** для
  локальной разработки. Без LTO быстрее, но и exe больше.
- **«Tauri лучше для Linux/macOS»** — для Kosmos с TipTap это **анти-аргумент**.
  Реальные приложения уровня Eden / Notion / Obsidian используют Electron
  именно потому, что Chromium стабилен везде.

## Стоимость гипотетической миграции

Из inventory Electron-поверхности Kosmos:

- **5814 строк Electron TS** в `shell/electron/` → переписать на Rust.
- **70 IPC handlers** → каждый = `#[tauri::command]` с serde-типами.
- **Preload bridge** (`window.kepler.*`, 4 extension'а) → JS shim над `__TAURI__`.
- **Extension dev-mode** (Vite probe + HMR) — переизобрести под Tauri.
- **`.kext` installer** (zip + path-traversal guards + semver) → Rust.
- **crashReporter** — потерять или построить с нуля.
- **autoUpdater** — потерять diff-update (electron-updater `.blockmap`).
- **focus-block / focus-service** (Win32 fullscreen + Windows Service) — Rust port.

Substantive port покрыл ~15% (9 IPC handlers, 2 окна, ~330 строк Rust + 120 JS).
По экстраполяции **полный feature-parity = 6-7× больше = 3-6 недель full-time на
Windows**, +4-8 недель на Linux/macOS обход WebKitGTK багов.

## Артефакты эксперимента

Эксперимент жил в эфемерной ветке `kosmos-tauri-test` (worktree). После
извлечения выводов в эту страницу ветка **удалена** — экспериментальный код
(`tauri-poc/`, `tauri-port/`) и черновики (`tauri-research/*.md`) в main не
мерджились намеренно. Всё, что осталось от эксперимента — этот документ и
ссылки на upstream-источники.

Если нужно повторить — воспроизводимо по описанию ниже.

## Воспроизведение

1. **Baseline Electron Kepler** (без сборки чего-либо нового):
   - Установить Kepler через NSIS installer.
   - Запустить, подождать стабилизации (3-4 секунды).
   - Снять process tree от PID `Kepler.exe (main)` через `Get-CimInstance Win32_Process`
     по `ParentProcessId`, просуммировать `WorkingSet64` всех.
   - Дополнительно отдельно посмотреть main / renderer / gpu / utility /
     crashpad по `--type=` в CommandLine.

2. **Tauri PoC** (для замера hello-world Tauri RAM/disk):
   ```powershell
   cargo install tauri-cli --version "^2.0" --locked  # 3-15 мин cold
   cargo tauri init --ci `
     --app-name kepler-tauri-poc `
     --window-title "Kepler PoC" `
     --frontend-dist ../src
   cargo tauri icon path/to/icon.png
   cargo tauri build --no-bundle
   ```
   Tauri.conf: `withGlobalTauri: true`, 1 окно 720×460, `transparent: true`,
   `decorations: false`. Cargo.toml release-profile: `lto = true`, `strip = true`,
   `opt-level = "s"`, `panic = "abort"`.

3. **Замер RAM Tauri PoC** через process tree от PID Tauri host'а
   (включит все `msedgewebview2.exe` дочерние процессы):
   ```powershell
   $p = Start-Process -FilePath '.\target\release\<app>.exe' -PassThru
   Start-Sleep -Seconds 4
   $all = Get-CimInstance Win32_Process
   $pids = @($p.Id); $queue = @($p.Id)
   while ($queue.Count -gt 0) {
     $next = @()
     foreach ($pp in $queue) {
       $kids = $all | Where-Object { $_.ParentProcessId -eq $pp }
       foreach ($c in $kids) { $pids += $c.ProcessId; $next += $c.ProcessId }
     }
     $queue = $next
   }
   $tot = 0
   foreach ($id in $pids) {
     $proc = Get-Process -Id $id -ErrorAction SilentlyContinue
     if ($proc) { $tot += $proc.WorkingSet64 }
   }
   "Total WS: {0:N1} MB" -f ($tot/1MB)
   ```

## Что пересмотреть в будущем

Этот результат **не вечен**. Что может изменить картину:

- **Servo-based webview в Tauri** (если/когда дозреет) — единый движок на всех ОС,
  без Chromium и WebKitGTK багов.
- **Tauri Mobile** становится зрелым — для kosmos-mobile это альтернатива
  Capacitor/native React Native.
- **electron-updater теряет diff-update в Win store** или Microsoft меняет
  политику — наш главный остающийся win Electron исчезает.
- **WebKitGTK чинит contentEditable** ([отслеживать в #12638](https://github.com/tauri-apps/tauri/issues/12638))
  — Linux перестаёт быть блокером для Eden.

Если что-то из этого случится — открыть новый эксперимент по этому же шаблону.
