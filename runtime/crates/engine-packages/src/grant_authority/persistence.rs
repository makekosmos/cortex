use super::*;

impl GrantAuthorityRegistry {
    fn record_path(&self) -> Option<PathBuf> {
        self.data_dir
            .as_ref()
            .map(|d| d.join("grant-authority.json"))
    }
    fn transaction_path(&self) -> Option<PathBuf> {
        self.data_dir
            .as_ref()
            .map(|d| d.join("legacy-grant-transactions.json"))
    }
    pub(super) fn load_records(&self) -> Result<Vec<PersistedRecord>, GrantError> {
        let Some(path) = self.record_path() else {
            return Ok(Vec::new());
        };
        let bytes = match fs::read(path) {
            Ok(v) => v,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(_) => return Err(GrantError::Persistence),
        };
        let records: Vec<PersistedRecord> =
            serde_json::from_slice(&bytes).map_err(|_| GrantError::Persistence)?;
        let mut ids = std::collections::HashSet::with_capacity(records.len());
        for record in &records {
            if record.version != 1
                || !Path::new(&record.selected_path).is_absolute()
                || record.persistent_grant_id.is_empty()
                || record.extension_id.is_empty()
                || !ids.insert(&record.persistent_grant_id)
            {
                return Err(GrantError::Persistence);
            }
        }
        Ok(records)
    }
    pub(super) fn save_records(&self, records: &[PersistedRecord]) -> Result<(), GrantError> {
        let Some(path) = self.record_path() else {
            return Ok(());
        };
        self.save_json(&path, records)
    }
    fn save_json<T: Serialize + ?Sized>(&self, path: &Path, value: &T) -> Result<(), GrantError> {
        let dir = path.parent().ok_or(GrantError::Persistence)?;
        fs::create_dir_all(dir).map_err(|_| GrantError::Persistence)?;
        let tmp = path.with_extension("json.tmp");
        let bytes = serde_json::to_vec(value).map_err(|_| GrantError::Persistence)?;
        {
            use std::io::Write;
            let mut f = fs::OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(&tmp)
                .map_err(|_| GrantError::Persistence)?;
            if f.write_all(&bytes).is_err() || f.sync_all().is_err() {
                let _ = fs::remove_file(&tmp);
                return Err(GrantError::Persistence);
            }
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600)).map_err(|_| {
                let _ = fs::remove_file(&tmp);
                GrantError::Persistence
            })?;
        }
        #[cfg(test)]
        if FAIL_PERSIST_AFTER_FSYNC.with(Cell::get) {
            let _ = fs::remove_file(&tmp);
            return Err(GrantError::Persistence);
        }
        fs::rename(&tmp, path).map_err(|_| {
            let _ = fs::remove_file(&tmp);
            GrantError::Persistence
        })?;
        #[cfg(unix)]
        {
            fs::File::open(dir)
                .and_then(|f| f.sync_all())
                .map_err(|_| GrantError::Persistence)?;
        }
        Ok(())
    }
    pub(super) fn load_transactions(&self) -> Result<Vec<LegacyGrantTransaction>, GrantError> {
        let path = self.transaction_path().ok_or(GrantError::Persistence)?;
        let bytes = match fs::read(path) {
            Ok(v) => v,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(_) => return Err(GrantError::Persistence),
        };
        if bytes.len() > MAX_GRANT_TRANSACTION_BYTES {
            return Err(GrantError::Persistence);
        }
        let transactions: Vec<LegacyGrantTransaction> =
            serde_json::from_slice(&bytes).map_err(|_| GrantError::Persistence)?;
        let mut tokens = std::collections::HashSet::with_capacity(transactions.len());
        for transaction in &transactions {
            if transaction.version != 1
                || transaction.token.is_empty()
                || transaction.token.len() > 128
                || !tokens.insert(&transaction.token)
                || transaction.source_ids.is_empty()
                || transaction.records.iter().any(|record| record.revoked)
            {
                return Err(GrantError::Persistence);
            }
        }
        Ok(transactions)
    }
    pub(super) fn save_transactions(
        &self,
        transactions: &[LegacyGrantTransaction],
    ) -> Result<(), GrantError> {
        let path = self.transaction_path().ok_or(GrantError::Persistence)?;
        self.save_json(&path, transactions)
    }
}
