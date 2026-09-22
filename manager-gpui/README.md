# manager-gpui

GPUI-порт Kosmos Manager — панель управления Engine без Electron-обвязки
(KOS-130). Покрывает тот же набор разделов и операций, что и Vue Manager,
через тот же контракт `/v1/rpc` + `/v1/health` + `/v1/info`.

## Запуск

```bash
cargo run --manifest-path manager-gpui/Cargo.toml
```

Приложение читает `engine.lock.json` из `KOSMOS_DATA_DIR` (по умолчанию
`~/.config/Kosmos` на Linux, `%APPDATA%\Kosmos` на Windows,
`~/Library/Application Support/Kosmos` на macOS) на каждый запрос — перезапуск
Engine подхватывается без рестарта приложения.

## Engine

Manager работает только поверх живого Engine (`kepler-backend`):

```bash
KOSMOS_DATA_DIR=/tmp/kosmos-dev cargo run -p kepler-backend
KOSMOS_DATA_DIR=/tmp/kosmos-dev cargo run --manifest-path manager-gpui/Cargo.toml
```

Если Engine не отвечает, внизу показывается баннер с ошибкой и кнопкой
«Обновить» — состояние не ломается, повторная загрузка поднимет данные.

## Покрытие (Vue → GPUI)

| Раздел | Операции |
| --- | --- |
| Данные | `manager.data.summary/types/list/search` |
| Синхронизация | `get_sync_snapshot`, `get_own_iroh_ticket`, `connect_with_pairing_code`, `disconnect_peer` |
| Маркетплейс | `store.catalog/refresh`, `packages.list/install/set_enabled/uninstall/trust_status/disclosure/refresh_catalog` |
| Движок | `engine.settings.get/set` (warm timeout, usage tracker) |
| Настройки | `manager.db_backups.list/create/validate/restore`, автозапуск (Host) |
| Интеграции | `integrations.list/set_credential/clear_credential/sync_now` |
| О приложении | `/v1/health`, `/v1/info`, `manager.diagnostics.snapshot/log_tail/support_bundle.*` |
| Обновления | `store.refresh`, `packages.refresh_catalog`, `packages.install` |
| Ключи | `dictation.get_config/verify_api_key/set_api_key/clear_api_key/test_connectivity/get_stats` |
| Браузер | `browser.json` `persistData` (тот же файл, что пишет Host) |
| Разработка | `packages.install_development`, `packages.uninstall` |

Host-only возможности (electron-updater, `dialog.showOpenDialog`,
`openPackage`, crash reports) остаются в Electron Host — в GPUI они помечены
как `Host`. См. матрицу паритета в `/home/box/devin-runs/kos-127/parity/`.

## Разработка

```bash
cargo fmt --manifest-path manager-gpui/Cargo.toml
cargo clippy --manifest-path manager-gpui/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path manager-gpui/Cargo.toml
```

`MANAGER_GPUI_OFFSCREEN=1` паркует окно за пределами экрана (smoke-прогоны).
