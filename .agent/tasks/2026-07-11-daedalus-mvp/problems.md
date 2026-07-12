# Problems

## Windows debug artifact lock

Первый `cargo test --manifest-path platform/runtime/Cargo.toml agents` не смог заменить `target/debug/kepler-backend.exe` (`os error 5`), потому что запущенный Kosmos держал workspace binary. Проверки перенесены в изолированный `CARGO_TARGET_DIR=.tmp/cargo-daedalus`; пользовательский процесс не останавливался.

## E2E strict locator

Первые Daedalus flow E2E дошли до Changes, approval, completed state и reopen, но широкие `getByText` совпали с несколькими корректно отображаемыми местами (`daedalus-e2e.txt` — file row/diff/cards, `Готово` — sidebar/header, title — sidebar/heading). Assertions сужены до exact file-row, `.status-pill` и heading; продуктовый код не менялся.

## Vite production reload

Headless E2E обнаружил пустой renderer после `reload`: `base: "./"` был ошибочно указан внутри `build`, поэтому production HTML содержал абсолютные `/assets/*`. `base` перенесён на верхний уровень Vite config; повторный strict E2E прошёл.

## Headless visual capture

Playwright `page.screenshot()` внутри Electron E2E завис на capture, хотя flow продолжал работать. Для визуальной проверки использован repo workflow: headless Chromium против Vite на `9917`, mocked preload/RPC и отдельные 1280×800/900×600 captures. Vite был остановлен по PID порта; видимые окна не запускались.
