// Legacy 0.9.x grant revocation transaction — migration-owned, tokened.

impl PackageService {
    /// Start the migration-owned grant transaction. Only the opaque token and
    /// count cross the package RPC boundary; record snapshots stay in runtime.
    pub fn revoke_legacy_grants_transaction(
        &self,
        source_ids: &[String],
    ) -> Result<crate::grant_authority::LegacyGrantRevocation, PackageError> {
        let source_ids = source_ids.iter().map(String::as_str).collect::<Vec<_>>();
        self.grants
            .revoke_legacy_records_transaction(&source_ids)
            .map_err(map_legacy_grant_error)
    }

    pub fn restore_migration_snapshot(
        &self,
        source_ids: &[String],
        transaction_token: Option<&str>,
    ) -> Result<crate::grant_authority::MigrationGrantRestoration, PackageError> {
        let source_ids = source_ids.iter().map(String::as_str).collect::<Vec<_>>();
        self.grants
            .restore_migration_snapshot(&source_ids, transaction_token)
            .map_err(map_legacy_grant_error)
    }

    pub fn commit_legacy_grants(&self, token: &str) -> Result<(), PackageError> {
        self.grants
            .commit_legacy_records(token)
            .map_err(map_legacy_grant_error)
    }
}

fn map_legacy_grant_error(error: crate::grant_authority::GrantError) -> PackageError {
    match error {
        crate::grant_authority::GrantError::Invalid
        | crate::grant_authority::GrantError::NotFound => PackageError::Invalid,
        _ => PackageError::Persistence,
    }
}
