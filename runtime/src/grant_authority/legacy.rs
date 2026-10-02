use super::*;

impl GrantAuthorityRegistry {
    /// Revoke persisted grants and durably retain only the records changed by
    /// this operation. The token is opaque; snapshots never leave the runtime.
    pub fn revoke_legacy_records_transaction(
        &self,
        source_ids: &[&str],
    ) -> Result<LegacyGrantRevocation, GrantError> {
        self.validate_legacy_records(source_ids)?;
        let _transaction_lock = self
            .legacy_transactions
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let mut records = self.load_records()?;
        let changed = records
            .iter()
            .filter(|record| source_ids.contains(&record.extension_id.as_str()) && !record.revoked)
            .cloned()
            .collect::<Vec<_>>();
        if changed.is_empty() {
            return Ok(LegacyGrantRevocation {
                transaction_token: None,
                revoked: 0,
            });
        }

        let token = uuid::Uuid::new_v4().to_string();
        let mut transactions = self.load_transactions()?;
        transactions.push(LegacyGrantTransaction {
            version: 1,
            token: token.clone(),
            source_ids: source_ids.iter().map(|id| (*id).to_owned()).collect(),
            records: changed,
            state: LegacyGrantTransactionState::Active,
        });
        // Snapshot first: a crash before the grant file write leaves a
        // recoverable transaction whose records are still unchanged.
        self.save_transactions(&transactions)?;

        let mut revoked = 0;
        for record in &mut records {
            if source_ids.contains(&record.extension_id.as_str()) && !record.revoked {
                record.revoked = true;
                revoked += 1;
            }
        }
        if let Err(error) = self.save_records(&records) {
            // Leave the active transaction durable; restore is idempotent and
            // will observe the original records after a restart.
            return Err(error);
        }

        let mut grants = self.grants.lock().unwrap_or_else(|p| p.into_inner());
        grants.retain(|_, grant| !source_ids.contains(&grant.extension_id.as_str()));
        Ok(LegacyGrantRevocation {
            transaction_token: Some(token),
            revoked,
        })
    }

    /// Compatibility wrapper for callers that only need the count. The
    /// durable transaction remains available to the migration API.
    pub fn revoke_legacy_records(&self, source_ids: &[&str]) -> Result<usize, GrantError> {
        self.revoke_legacy_records_transaction(source_ids)
            .map(|result| result.revoked)
    }

    /// Restore exactly the records captured by the typed migration snapshot.
    ///
    /// The source set is part of the request so restart recovery cannot
    /// restore a token under a different migration. With no token, resolve an
    /// active snapshot by source set; this covers a crash after grant mutation
    /// but before the host journal persisted the opaque token.
    pub fn restore_migration_snapshot(
        &self,
        source_ids: &[&str],
        transaction_token: Option<&str>,
    ) -> Result<MigrationGrantRestoration, GrantError> {
        self.validate_legacy_records(source_ids)?;
        if let Some(token) = transaction_token {
            Self::validate_transaction_token(token)?;
        }
        let _transaction_lock = self
            .legacy_transactions
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let mut transactions = self.load_transactions()?;
        let matches_sources = |transaction: &LegacyGrantTransaction| {
            transaction
                .source_ids
                .iter()
                .map(String::as_str)
                .eq(source_ids.iter().copied())
        };
        let transaction_index = match transaction_token {
            Some(token) => transactions
                .iter()
                .position(|transaction| transaction.token == token && matches_sources(transaction)),
            None => transactions.iter().position(|transaction| {
                transaction.state == LegacyGrantTransactionState::Active
                    && matches_sources(transaction)
            }),
        };
        let Some(transaction_index) = transaction_index else {
            if transaction_token.is_some() {
                return Err(GrantError::NotFound);
            }
            return Ok(MigrationGrantRestoration {
                snapshot_found: false,
                restored: 0,
            });
        };
        let transaction = &mut transactions[transaction_index];
        if transaction.state == LegacyGrantTransactionState::Committed {
            return Err(GrantError::Invalid);
        }
        if transaction.state == LegacyGrantTransactionState::Restored {
            return Ok(MigrationGrantRestoration {
                snapshot_found: true,
                restored: 0,
            });
        }

        let mut records = self.load_records()?;
        let mut restored = 0;
        for snapshot in &transaction.records {
            let current = records
                .iter_mut()
                .find(|record| record.persistent_grant_id == snapshot.persistent_grant_id)
                .ok_or(GrantError::Persistence)?;
            if *current == *snapshot {
                continue;
            }
            let mut expected = snapshot.clone();
            expected.revoked = true;
            if *current != expected {
                return Err(GrantError::Persistence);
            }
            *current = snapshot.clone();
            restored += 1;
        }
        if restored > 0 {
            self.save_records(&records)?;
        }
        transaction.state = LegacyGrantTransactionState::Restored;
        self.save_transactions(&transactions)?;
        Ok(MigrationGrantRestoration {
            snapshot_found: true,
            restored,
        })
    }

    /// Mark a transaction successful. Committed transactions cannot be
    /// restored, preventing a later restart from resurrecting legacy grants.
    pub fn commit_legacy_records(&self, token: &str) -> Result<(), GrantError> {
        Self::validate_transaction_token(token)?;
        let _transaction_lock = self
            .legacy_transactions
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let mut transactions = self.load_transactions()?;
        let transaction = transactions
            .iter_mut()
            .find(|transaction| transaction.token == token)
            .ok_or(GrantError::NotFound)?;
        match transaction.state {
            LegacyGrantTransactionState::Active => {
                transaction.state = LegacyGrantTransactionState::Committed;
                self.save_transactions(&transactions)
            }
            LegacyGrantTransactionState::Committed => Ok(()),
            LegacyGrantTransactionState::Restored => Err(GrantError::Invalid),
        }
    }

    fn validate_transaction_token(token: &str) -> Result<(), GrantError> {
        if token.is_empty() || token.len() > 128 {
            Err(GrantError::Invalid)
        } else {
            Ok(())
        }
    }
    pub fn validate_legacy_records(&self, source_ids: &[&str]) -> Result<(), GrantError> {
        if source_ids.is_empty()
            || source_ids
                .iter()
                .any(|id| !LEGACY_EXTENSION_IDS.contains(id))
            || source_ids
                .iter()
                .enumerate()
                .any(|(index, id)| source_ids[..index].contains(id))
        {
            return Err(GrantError::Invalid);
        }
        self.load_records().map(|_| ())
    }
}
