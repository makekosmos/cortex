use super::{CommandClass, Ledger};

pub(super) fn has_maintenance_barrier(ledger: &Ledger) -> bool {
    ledger.maintenance_active
        || ledger
            .slots
            .iter()
            .flatten()
            .any(|slot| slot.class == CommandClass::Maintenance)
}
