// Kosmos Kepler backend — headless ARK host runtime.
//
// Этот крейт extracted из apps/kepler/ в Phase 0 Kepler-Electron pivot. Содержит
// весь backend layer без UI: WS server, ark-core-rpc supervisor, singleton,
// lock-file discovery, протокол handshake, auth, LAN sync wiring.
//
// Используется двумя binaries:
//   * `kepler-backend` (этот крейт) — headless, для Electron Kepler child.
//   * `kepler` (apps/kepler/) — legacy desktop binary с tray + launcher.
//
// Оба binary запускают идентичный setup; UI binary добавляет tray + launcher
// поверх; headless binary только ждёт Ctrl+C / parent kill.

pub mod ark_host;
pub mod auth;
pub mod command_bus;
pub mod lock_file;
pub mod protocol_version;
pub mod singleton;
pub mod sync;
pub mod usage_tracker;
pub mod ws_server;
