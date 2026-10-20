use rusqlite::Connection;

pub use ark_application::migration_source_projection::{
    ObjectPlan, ObjectPlanError, PlannedItem, CONTRACT_VERSION,
};

/// Read source rows through Store and delegate deterministic planning to Application.
pub fn plan_objects(conn: &Connection, now: &str) -> Result<ObjectPlan, ObjectPlanError> {
    let snapshot =
        crate::source_snapshot::read_source_snapshot(conn).map_err(ObjectPlanError::Storage)?;
    ark_application::migration_source_projection::plan_objects_from_snapshot(&snapshot, now)
}

pub fn apply_plan(conn: &Connection, plan: &ObjectPlan, now: &str) -> Result<(), ObjectPlanError> {
    super::execute::apply_plan_with_failure(conn, plan, now, None)
}
