// Kosmos Kepler — desktop binary с tray + launcher.
//
// После Phase 0 Electron-pivot backend модули живут в services/kepler-backend/
// (см. kepler-backend крейт). Этот lib.rs re-export'ит backend public API под
// привычными путями `kepler::ark_host`, `kepler::lock_file` и т.д., чтобы
// integration-тесты и legacy callers продолжали работать без изменения import'ов.

pub use kepler_backend::{
    ark_host, auth, lock_file, protocol_version, singleton, sync, ws_server,
};

pub mod launcher;
pub mod tray;
