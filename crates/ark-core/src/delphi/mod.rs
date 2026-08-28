//! Delphi-specific business logic, ported from `products/delphi/src/services/`.
//!
//! Каждый submodule = port одного TS service. Public API через ARK
//! operations в `crate::main` (или Делphi-направленный сервер).
//!
//! См. proof loop `.agent/tasks/2026-05-15-render-vs-rust-wave1/` для
//! parity-criteria с TS baseline.

pub mod filters;
pub mod types;
