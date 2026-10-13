pub mod compatibility;
pub mod core_task_rewrite;
pub mod definitions;
pub mod facades;
pub mod game;
pub mod ingress;
pub mod migration;
pub mod migration_ledger;
pub mod migration_objects;
pub mod migration_registry;
pub mod normalize;
pub mod pending;
pub mod preflight;
pub mod validation;

#[cfg(test)]
mod core_task_rewrite_tests;
