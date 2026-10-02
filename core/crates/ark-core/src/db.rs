include!("db/open.rs");
include!("db/snapshot.rs");
include!("db/legacy.rs");
include!("db/objects.rs");
include!("db/usage.rs");
include!("db/sync.rs");
include!("db/compact.rs");
include!("db/load.rs");
include!("db/backend.rs");
include!("db/integration_replication_common.rs");
include!("db/integration_replication_records.rs");
include!("db/integration_replication_grants.rs");
include!("db/integration_replication_credentials.rs");
include!("db/integration_replication_issuer_lookup.rs");
include!("db/integration_replication_lease.rs");
include!("db/integration_replication_publication.rs");
include!("db/integration_replication_export.rs");
include!("db/integration_replication_admission.rs");
include!("db/integration_replication_apply.rs");

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    include!("db/tests_core.rs");
    include!("db/tests_snapshot.rs");
    include!("db/tests_sync.rs");
    include!("db/tests_integration_replication.rs");
    include!("db/tests_integration_replication_issuer_lookup.rs");
    include!("db/tests_integration_replication_issuer_publish_lookup.rs");
    include!("db/tests_integration_replication_export.rs");
    include!("db/tests_integration_replication_apply.rs");
    include!("db/tests_integration_replication_security.rs");
    include!("db/tests_integration_replication_publication.rs");
}
