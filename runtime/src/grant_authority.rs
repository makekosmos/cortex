use crate::handle_relative_fs::{self, RootHandle, RootIdentity};
use serde::{Deserialize, Serialize};
#[cfg(test)]
use std::cell::Cell;
use std::{
    collections::{BTreeMap, HashMap},
    fs, io,
    path::{Path, PathBuf},
    sync::Mutex,
};

#[cfg(test)]
thread_local! {
    static FAIL_PERSIST_AFTER_FSYNC: Cell<bool> = const { Cell::new(false) };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GrantProvenance {
    NativeDialog,
    PersistedUserData,
}
impl GrantProvenance {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "native-dialog" => Some(Self::NativeDialog),
            "persisted" | "persisted-userdata" => Some(Self::PersistedUserData),
            _ => None,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NativeDialog => "native-dialog",
            Self::PersistedUserData => "persisted",
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrantOwner {
    pub session_id: String,
    pub generation: u64,
    pub connection_id: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct PersistedRecord {
    version: u32,
    persistent_grant_id: String,
    extension_id: String,
    provenance: GrantProvenance,
    exact_file: bool,
    selected_path: String,
    root_identity: RootIdentity,
    exact_file_identity: Option<RootIdentity>,
    revoked: bool,
    #[serde(flatten)]
    extra: BTreeMap<String, serde_json::Value>,
}
struct Grant {
    owner: GrantOwner,
    extension_id: String,
    exact_file: bool,
    selected_name: Option<String>,
    exact_file_identity: Option<RootIdentity>,
    root: RootHandle,
    identity: RootIdentity,
    persistent_id: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GrantError {
    Invalid,
    NotFound,
    OwnerMismatch,
    ExtensionMismatch,
    ScopeMismatch,
    IdentityChanged,
    Persistence,
}
const MAX_GRANT_FILE_BYTES: usize = 1024 * 1024;
const MAX_GRANT_DIRECTORY_ENTRIES: usize = 4096;
const MAX_GRANT_TRANSACTION_BYTES: usize = 16 * 1024 * 1024;
const LEGACY_EXTENSION_IDS: &[&str] = &["arcadia", "arrancador", "eden", "delphi"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyGrantRevocation {
    pub transaction_token: Option<String>,
    pub revoked: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationGrantRestoration {
    pub snapshot_found: bool,
    pub restored: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum LegacyGrantTransactionState {
    Active,
    Restored,
    Committed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyGrantTransaction {
    version: u32,
    token: String,
    source_ids: Vec<String>,
    records: Vec<PersistedRecord>,
    state: LegacyGrantTransactionState,
}

pub struct GrantAuthorityRegistry {
    grants: Mutex<HashMap<String, Grant>>,
    legacy_transactions: Mutex<()>,
    data_dir: Option<PathBuf>,
}
impl Default for GrantAuthorityRegistry {
    fn default() -> Self {
        Self {
            grants: Mutex::new(HashMap::new()),
            legacy_transactions: Mutex::new(()),
            data_dir: None,
        }
    }
}
impl GrantAuthorityRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn with_data_dir(data_dir: PathBuf) -> Self {
        Self {
            grants: Mutex::new(HashMap::new()),
            legacy_transactions: Mutex::new(()),
            data_dir: Some(data_dir),
        }
    }
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
    fn load_records(&self) -> Result<Vec<PersistedRecord>, GrantError> {
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
    fn save_records(&self, records: &[PersistedRecord]) -> Result<(), GrantError> {
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
    fn load_transactions(&self) -> Result<Vec<LegacyGrantTransaction>, GrantError> {
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
    fn save_transactions(&self, transactions: &[LegacyGrantTransaction]) -> Result<(), GrantError> {
        let path = self.transaction_path().ok_or(GrantError::Persistence)?;
        self.save_json(&path, transactions)
    }
    fn open_selected(
        path: &Path,
        exact_file: bool,
    ) -> Result<
        (
            RootHandle,
            RootIdentity,
            Option<String>,
            Option<RootIdentity>,
        ),
        GrantError,
    > {
        if !path.is_absolute() {
            return Err(GrantError::Invalid);
        }
        let (root_path, name) = if exact_file {
            (
                path.parent().ok_or(GrantError::Invalid)?,
                Some(
                    path.file_name()
                        .and_then(|x| x.to_str())
                        .ok_or(GrantError::Invalid)?
                        .to_owned(),
                ),
            )
        } else {
            (path, None)
        };
        let root = handle_relative_fs::open_root(root_path).map_err(|_| GrantError::Invalid)?;
        let identity = handle_relative_fs::root_identity(&root).map_err(|_| GrantError::Invalid)?;
        let file_identity = if exact_file {
            Some(
                handle_relative_fs::file_identity(
                    &root,
                    name.as_deref().ok_or(GrantError::Invalid)?,
                )
                .map_err(|_| GrantError::Invalid)?,
            )
        } else {
            None
        };
        Ok((root, identity, name, file_identity))
    }
    pub fn register(
        &self,
        owner: &GrantOwner,
        extension_id: &str,
        selected_root: &Path,
        exact_file: bool,
        provenance: GrantProvenance,
        persisted_version: Option<u64>,
    ) -> Result<(String, RootIdentity, Option<String>), GrantError> {
        if extension_id.is_empty() {
            return Err(GrantError::Invalid);
        }
        let (root, identity, selected_name, file_identity) =
            Self::open_selected(selected_root, exact_file)?;
        let persistent_id =
            (provenance == GrantProvenance::NativeDialog).then(|| uuid::Uuid::new_v4().to_string());
        if let Some(ref id) = persistent_id {
            let mut records = self.load_records()?;
            records.retain(|r| r.persistent_grant_id != *id);
            records.push(PersistedRecord {
                version: persisted_version.unwrap_or(1) as u32,
                persistent_grant_id: id.clone(),
                extension_id: extension_id.to_owned(),
                provenance,
                exact_file,
                selected_path: selected_root.to_string_lossy().into_owned(),
                root_identity: identity,
                exact_file_identity: file_identity,
                revoked: false,
                extra: BTreeMap::new(),
            });
            self.save_records(&records)?;
        }
        let id = uuid::Uuid::new_v4().to_string();
        self.grants
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(
                id.clone(),
                Grant {
                    owner: owner.clone(),
                    extension_id: extension_id.to_owned(),
                    exact_file,
                    selected_name,
                    exact_file_identity: file_identity,
                    root,
                    identity,
                    persistent_id: persistent_id.clone(),
                },
            );
        Ok((id, identity, persistent_id))
    }
    pub fn reopen(
        &self,
        owner: &GrantOwner,
        persistent_id: &str,
        extension_id: &str,
    ) -> Result<(String, RootIdentity), GrantError> {
        let record = self
            .load_records()?
            .into_iter()
            .find(|r| r.persistent_grant_id == persistent_id && !r.revoked)
            .ok_or(GrantError::NotFound)?;
        if record.extension_id != extension_id || record.provenance != GrantProvenance::NativeDialog
        {
            return Err(GrantError::ExtensionMismatch);
        }
        let (root, identity, selected_name, file_identity) =
            Self::open_selected(Path::new(&record.selected_path), record.exact_file)?;
        if identity != record.root_identity || file_identity != record.exact_file_identity {
            return Err(GrantError::IdentityChanged);
        }
        let id = uuid::Uuid::new_v4().to_string();
        self.grants
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(
                id.clone(),
                Grant {
                    owner: owner.clone(),
                    extension_id: extension_id.to_owned(),
                    exact_file: record.exact_file,
                    selected_name,
                    exact_file_identity: file_identity,
                    root,
                    identity,
                    persistent_id: Some(persistent_id.to_owned()),
                },
            );
        Ok((id, identity))
    }
    pub fn read(
        &self,
        grant_id: &str,
        owner: &GrantOwner,
        extension_id: &str,
        requested: &[&str],
        max_bytes: usize,
    ) -> Result<Vec<u8>, GrantError> {
        self.with_authorized_grant(grant_id, owner, extension_id, requested, true, |grant| {
            handle_relative_fs::read_relative(&grant.root, requested, max_bytes)
                .map_err(|_| GrantError::ScopeMismatch)
        })
    }
    fn with_authorized_grant<T>(
        &self,
        grant_id: &str,
        owner: &GrantOwner,
        extension_id: &str,
        requested: &[&str],
        exact_file_allowed: bool,
        operation: impl FnOnce(&Grant) -> Result<T, GrantError>,
    ) -> Result<T, GrantError> {
        let grants = self.grants.lock().unwrap_or_else(|p| p.into_inner());
        let grant = grants.get(grant_id).ok_or(GrantError::NotFound)?;
        if &grant.owner != owner {
            return Err(GrantError::OwnerMismatch);
        }
        if grant.extension_id != extension_id {
            return Err(GrantError::ExtensionMismatch);
        }
        if handle_relative_fs::root_identity(&grant.root)
            .map_err(|_| GrantError::IdentityChanged)?
            != grant.identity
        {
            return Err(GrantError::IdentityChanged);
        }
        if grant.exact_file && !exact_file_allowed {
            return Err(GrantError::ScopeMismatch);
        }
        if grant.exact_file
            && (requested.len() != 1
                || grant.selected_name.as_deref() != requested.first().copied())
        {
            return Err(GrantError::ScopeMismatch);
        }
        if grant.exact_file {
            let selected_name = grant
                .selected_name
                .as_deref()
                .ok_or(GrantError::IdentityChanged)?;
            let expected = grant
                .exact_file_identity
                .ok_or(GrantError::IdentityChanged)?;
            let current = handle_relative_fs::file_identity(&grant.root, selected_name)
                .map_err(|_| GrantError::IdentityChanged)?;
            if current != expected {
                return Err(GrantError::IdentityChanged);
            }
        }
        operation(grant)
    }
    pub fn write(
        &self,
        grant_id: &str,
        owner: &GrantOwner,
        extension_id: &str,
        requested: &[&str],
        payload: &[u8],
    ) -> Result<(), GrantError> {
        if payload.len() > MAX_GRANT_FILE_BYTES {
            return Err(GrantError::Invalid);
        }
        self.with_authorized_grant(grant_id, owner, extension_id, requested, false, |grant| {
            handle_relative_fs::write_relative(
                &grant.root,
                requested,
                payload,
                MAX_GRANT_FILE_BYTES,
            )
            .map_err(|_| GrantError::ScopeMismatch)
        })
    }
    pub fn list(
        &self,
        grant_id: &str,
        owner: &GrantOwner,
        extension_id: &str,
        requested: &[&str],
    ) -> Result<Vec<handle_relative_fs::RelativeEntry>, GrantError> {
        let mut entries =
            self.with_authorized_grant(grant_id, owner, extension_id, requested, false, |grant| {
                handle_relative_fs::list_relative(
                    &grant.root,
                    requested,
                    MAX_GRANT_DIRECTORY_ENTRIES,
                )
                .map_err(|_| GrantError::ScopeMismatch)
            })?;
        entries.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(entries)
    }
    pub fn delete(
        &self,
        grant_id: &str,
        owner: &GrantOwner,
        extension_id: &str,
        requested: &[&str],
    ) -> Result<(), GrantError> {
        self.with_authorized_grant(grant_id, owner, extension_id, requested, false, |grant| {
            handle_relative_fs::delete_relative(&grant.root, requested)
                .map_err(|_| GrantError::ScopeMismatch)
        })
    }
    pub fn mkdir(
        &self,
        grant_id: &str,
        owner: &GrantOwner,
        extension_id: &str,
        requested: &[&str],
    ) -> Result<(), GrantError> {
        self.with_authorized_grant(grant_id, owner, extension_id, requested, false, |grant| {
            handle_relative_fs::mkdir_relative(&grant.root, requested)
                .map_err(|_| GrantError::ScopeMismatch)
        })
    }
    pub fn close_owner(&self, owner: GrantOwner) -> usize {
        let mut grants = self.grants.lock().unwrap_or_else(|p| p.into_inner());
        let before = grants.len();
        grants.retain(|_, g| g.owner != owner);
        before - grants.len()
    }
    pub fn close_generation(&self, session_id: &str, generation: u64) -> usize {
        let mut grants = self.grants.lock().unwrap_or_else(|p| p.into_inner());
        let before = grants.len();
        grants
            .retain(|_, g| !(g.owner.session_id == session_id && g.owner.generation == generation));
        before - grants.len()
    }
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
    pub fn len(&self) -> usize {
        self.grants.lock().unwrap_or_else(|p| p.into_inner()).len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn owner(n: u64) -> GrantOwner {
        GrantOwner {
            session_id: "s".into(),
            generation: n,
            connection_id: n,
        }
    }
    #[test]
    fn persisted_reopen_same_identity_reads_bytes_and_owner_is_fenced() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        fs::write(&file, b"bytes").unwrap();
        let reg = GrantAuthorityRegistry::with_data_dir(dir.path().join("engine"));
        let old = owner(1);
        let (_, _, persistent) = reg
            .register(
                &old,
                "ext",
                &file,
                true,
                GrantProvenance::NativeDialog,
                None,
            )
            .unwrap();
        let persistent = persistent.unwrap();
        assert_eq!(reg.close_owner(old), 1);
        let new = owner(2);
        let (id, _) = reg.reopen(&new, &persistent, "ext").unwrap();
        assert_eq!(
            reg.read(&id, &new, "ext", &["a.txt"], 64).unwrap(),
            b"bytes"
        );
        assert_eq!(
            reg.read(&id, &owner(1), "ext", &["a.txt"], 64),
            Err(GrantError::OwnerMismatch)
        );
    }
    #[test]
    fn replacement_root_and_exact_file_are_denied() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("root");
        fs::create_dir(&root).unwrap();
        let file = root.join("a.txt");
        fs::write(&file, b"a").unwrap();
        let reg = GrantAuthorityRegistry::with_data_dir(dir.path().join("engine"));
        let o = owner(1);
        let (_, _, p) = reg
            .register(&o, "ext", &root, false, GrantProvenance::NativeDialog, None)
            .unwrap();
        let p = p.unwrap();
        fs::rename(&root, dir.path().join("A")).unwrap();
        fs::create_dir(&root).unwrap();
        assert_eq!(reg.reopen(&o, &p, "ext"), Err(GrantError::IdentityChanged));
        let root2 = dir.path().join("root2");
        fs::create_dir(&root2).unwrap();
        let f = root2.join("x");
        fs::write(&f, b"a").unwrap();
        let (_, _, p2) = reg
            .register(&o, "ext", &f, true, GrantProvenance::NativeDialog, None)
            .unwrap();
        let p2 = p2.unwrap();
        fs::remove_file(&f).unwrap();
        fs::write(&f, b"b").unwrap();
        assert_eq!(reg.reopen(&o, &p2, "ext"), Err(GrantError::IdentityChanged));
    }
    #[test]
    fn registered_exact_file_denies_replacement_before_read() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        fs::write(&file, b"old").unwrap();
        let reg = GrantAuthorityRegistry::new();
        let o = owner(1);
        let (id, _, _) = reg
            .register(&o, "ext", &file, true, GrantProvenance::NativeDialog, None)
            .unwrap();
        fs::remove_file(&file).unwrap();
        fs::write(&file, b"replacement").unwrap();

        assert_eq!(
            reg.read(&id, &o, "ext", &["a.txt"], 64),
            Err(GrantError::IdentityChanged)
        );
    }
    #[test]
    fn directory_grant_supports_bounded_write_list_delete_and_mkdir() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("root");
        fs::create_dir(&root).unwrap();
        let reg = GrantAuthorityRegistry::new();
        let o = owner(1);
        let (id, _, _) = reg
            .register(&o, "ext", &root, false, GrantProvenance::NativeDialog, None)
            .unwrap();

        reg.mkdir(&id, &o, "ext", &["nested"]).unwrap();
        reg.write(&id, &o, "ext", &["nested", "note.txt"], b"hello")
            .unwrap();
        let entries = reg.list(&id, &o, "ext", &["nested"]).unwrap();
        assert_eq!(
            entries,
            vec![handle_relative_fs::RelativeEntry {
                name: "note.txt".into(),
                directory: false,
            }]
        );
        assert_eq!(
            reg.read(&id, &o, "ext", &["nested", "note.txt"], 64)
                .unwrap(),
            b"hello"
        );
        reg.delete(&id, &o, "ext", &["nested", "note.txt"]).unwrap();
        assert!(reg.list(&id, &o, "ext", &["nested"]).unwrap().is_empty());
    }
    #[test]
    fn directory_grant_rejects_traversal_and_root_replacement() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("root");
        fs::create_dir(&root).unwrap();
        let reg = GrantAuthorityRegistry::new();
        let o = owner(1);
        let (id, _, _) = reg
            .register(&o, "ext", &root, false, GrantProvenance::NativeDialog, None)
            .unwrap();
        assert!(reg.write(&id, &o, "ext", &["..", "escape"], b"x").is_err());
        assert!(reg.list(&id, &o, "ext", &["a/b"]).is_err());

        let old_root = dir.path().join("root.old");
        fs::rename(&root, &old_root).unwrap();
        fs::create_dir(&root).unwrap();
        reg.write(&id, &o, "ext", &["safe"], b"pinned").unwrap();
        assert_eq!(fs::read(old_root.join("safe")).unwrap(), b"pinned");
        assert!(!root.join("safe").exists());
    }
    #[test]
    fn exact_file_grant_keeps_directory_operations_out_of_scope() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("only.txt");
        fs::write(&file, b"old").unwrap();
        let reg = GrantAuthorityRegistry::new();
        let o = owner(1);
        let (id, _, _) = reg
            .register(&o, "ext", &file, true, GrantProvenance::NativeDialog, None)
            .unwrap();
        assert!(reg.list(&id, &o, "ext", &[]).is_err());
        assert!(reg.mkdir(&id, &o, "ext", &["nested"]).is_err());
        assert!(reg.write(&id, &o, "ext", &["other.txt"], b"no").is_err());
        assert!(reg.write(&id, &o, "ext", &["only.txt"], b"new").is_err());
        assert!(reg.delete(&id, &o, "ext", &["only.txt"]).is_err());
        assert_eq!(fs::read(file).unwrap(), b"old");
    }
    #[cfg(unix)]
    #[test]
    fn root_symlink_and_child_symlink_are_denied() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real");
        fs::create_dir(&real).unwrap();
        fs::write(real.join("a"), b"a").unwrap();
        symlink(&real, dir.path().join("link")).unwrap();
        let reg = GrantAuthorityRegistry::new();
        let o = owner(1);
        assert_eq!(
            reg.register(
                &o,
                "ext",
                &dir.path().join("link"),
                false,
                GrantProvenance::NativeDialog,
                None
            ),
            Err(GrantError::Invalid)
        );
        symlink(real.join("a"), real.join("b")).unwrap();
        let (id, _, _) = reg
            .register(&o, "ext", &real, false, GrantProvenance::NativeDialog, None)
            .unwrap();
        assert!(reg.read(&id, &o, "ext", &["b"], 64).is_err());
    }
    #[test]
    fn ten_thousand_reads_keep_cardinality_bounded() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("a");
        fs::write(&f, b"a").unwrap();
        let reg = GrantAuthorityRegistry::new();
        let o = owner(1);
        let (id, _, _) = reg
            .register(&o, "ext", &f, true, GrantProvenance::NativeDialog, None)
            .unwrap();
        for _ in 0..10_000 {
            assert_eq!(reg.read(&id, &o, "ext", &["a"], 8).unwrap(), b"a");
        }
        assert_eq!(reg.len(), 1);
        assert_eq!(reg.close_owner(o), 1);
        assert_eq!(reg.len(), 0);
    }

    #[test]
    fn generation_close_releases_only_matching_handles() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a");
        fs::write(&file, b"a").unwrap();
        let reg = GrantAuthorityRegistry::new();
        for generation in [1, 2] {
            reg.register(
                &owner(generation),
                "ext",
                &file,
                true,
                GrantProvenance::NativeDialog,
                None,
            )
            .unwrap();
        }

        assert_eq!(reg.close_generation("s", 1), 1);
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn persistence_failure_after_fsync_preserves_previous_record_and_redacts_errors() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a");
        fs::write(&file, b"a").unwrap();
        let data = dir.path().join("engine");
        let reg = GrantAuthorityRegistry::with_data_dir(data.clone());
        let o = owner(1);
        let (_, _, persistent) = reg
            .register(&o, "ext", &file, true, GrantProvenance::NativeDialog, None)
            .unwrap();
        let path = data.join("grant-authority.json");
        let before = fs::read(&path).unwrap();
        FAIL_PERSIST_AFTER_FSYNC.with(|fail| fail.set(true));
        let result = reg.register(&o, "ext", &file, true, GrantProvenance::NativeDialog, None);
        FAIL_PERSIST_AFTER_FSYNC.with(|fail| fail.set(false));
        assert_eq!(result, Err(GrantError::Persistence));
        assert_eq!(fs::read(&path).unwrap(), before);
        assert!(!path.with_extension("json.tmp").exists());
        let reopened = GrantAuthorityRegistry::with_data_dir(data);
        assert!(reopened
            .reopen(&owner(2), persistent.as_deref().unwrap(), "ext")
            .is_ok());
        fs::write(
            &path,
            br#"[{"version":99,"selected_path":"/secret","extension_id":"ext"}]"#,
        )
        .unwrap();
        let error = reopened.reopen(&owner(2), "secret", "ext").unwrap_err();
        let text = format!("{error:?}");
        assert!(!text.contains("/secret"));
        assert!(!text.contains("ext"));
    }

    #[test]
    fn legacy_revoke_preserves_unknown_fields_and_unrelated_records() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("grant-authority.json");
        let records = serde_json::json!([
            {
                "version": 1,
                "persistent_grant_id": "legacy",
                "extension_id": "eden",
                "provenance": "NativeDialog",
                "exact_file": false,
                "selected_path": dir.path().to_string_lossy(),
                "root_identity": {"primary": 1, "secondary": 2},
                "exact_file_identity": null,
                "revoked": false,
                "future_scope": {"read": ["a"], "write": null},
                "future_marker": "preserve-me"
            },
            {
                "version": 1,
                "persistent_grant_id": "unrelated",
                "extension_id": "other",
                "provenance": "NativeDialog",
                "exact_file": false,
                "selected_path": dir.path().to_string_lossy(),
                "root_identity": {"primary": 1, "secondary": 2},
                "exact_file_identity": null,
                "revoked": false,
                "future_marker": "untouched"
            }
        ]);
        fs::write(&path, serde_json::to_vec(&records).unwrap()).unwrap();

        let revoked = GrantAuthorityRegistry::with_data_dir(dir.path().to_path_buf())
            .revoke_legacy_records(&["eden"])
            .unwrap();
        assert_eq!(revoked, 1);

        let persisted: serde_json::Value =
            serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        assert_eq!(persisted[0]["revoked"], true);
        assert_eq!(persisted[0]["future_scope"]["read"][0], "a");
        assert_eq!(persisted[0]["future_marker"], "preserve-me");
        assert_eq!(persisted[1]["revoked"], false);
        assert_eq!(persisted[1]["future_marker"], "untouched");
    }

    #[test]
    fn malformed_records_abort_legacy_revoke_before_write_or_handle_clear() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("grant-authority.json");
        let before = br#"[{"version":1,"persistent_grant_id":"legacy","extension_id":"eden","provenance":"native-dialog","exact_file":false,"selected_path":"/tmp","root_identity":{"dev":1,"ino":2},"exact_file_identity":null,"revoked":false},{"version":99}]"#;
        fs::write(&path, before).unwrap();

        let registry = GrantAuthorityRegistry::with_data_dir(dir.path().to_path_buf());
        assert_eq!(
            registry.revoke_legacy_records(&["eden"]),
            Err(GrantError::Persistence)
        );
        assert_eq!(fs::read(&path).unwrap(), before);
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn legacy_revoke_clears_only_live_handles_for_allowlisted_owners() {
        let dir = tempfile::tempdir().unwrap();
        let legacy_root = dir.path().join("legacy");
        let unrelated_root = dir.path().join("unrelated");
        fs::create_dir(&legacy_root).unwrap();
        fs::create_dir(&unrelated_root).unwrap();
        let registry = GrantAuthorityRegistry::with_data_dir(dir.path().join("engine"));
        registry
            .register(
                &owner(1),
                "eden",
                &legacy_root,
                false,
                GrantProvenance::NativeDialog,
                None,
            )
            .unwrap();
        registry
            .register(
                &owner(2),
                "other",
                &unrelated_root,
                false,
                GrantProvenance::NativeDialog,
                None,
            )
            .unwrap();

        assert_eq!(registry.revoke_legacy_records(&["eden"]).unwrap(), 1);
        assert_eq!(registry.len(), 1);
        assert_eq!(registry.close_owner(owner(2)), 1);
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn scoped_legacy_revoke_leaves_other_legacy_identity_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("grant-authority.json");
        let records = serde_json::json!([
            {
                "version": 1,
                "persistent_grant_id": "eden-grant",
                "extension_id": "eden",
                "provenance": "NativeDialog",
                "exact_file": false,
                "selected_path": dir.path().to_string_lossy(),
                "root_identity": {"primary": 1, "secondary": 2},
                "exact_file_identity": null,
                "revoked": false,
                "unknown": "keep"
            },
            {
                "version": 1,
                "persistent_grant_id": "delphi-grant",
                "extension_id": "delphi",
                "provenance": "NativeDialog",
                "exact_file": false,
                "selected_path": dir.path().to_string_lossy(),
                "root_identity": {"primary": 1, "secondary": 2},
                "exact_file_identity": null,
                "revoked": false,
                "unknown": "untouched"
            }
        ]);
        fs::write(&path, serde_json::to_vec(&records).unwrap()).unwrap();

        assert_eq!(
            GrantAuthorityRegistry::with_data_dir(dir.path().to_path_buf())
                .revoke_legacy_records(&["eden"])
                .unwrap(),
            1
        );
        let persisted: serde_json::Value =
            serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        assert_eq!(persisted[0]["persistent_grant_id"], "eden-grant");
        assert_eq!(persisted[0]["revoked"], true);
        assert_eq!(persisted[0]["unknown"], "keep");
        assert_eq!(persisted[1]["persistent_grant_id"], "delphi-grant");
        assert_eq!(persisted[1]["revoked"], false);
        assert_eq!(persisted[1]["unknown"], "untouched");
    }

    #[test]
    fn invalid_scope_is_rejected_without_touching_persisted_records() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("grant-authority.json");
        let before = br#"[{"version":1,"persistent_grant_id":"legacy","extension_id":"eden","provenance":"native-dialog","exact_file":false,"selected_path":"/tmp","root_identity":{"dev":1,"ino":2},"exact_file_identity":null,"revoked":false}]"#;
        fs::write(&path, before).unwrap();

        let registry = GrantAuthorityRegistry::with_data_dir(dir.path().to_path_buf());
        assert_eq!(
            registry.revoke_legacy_records(&["eden", "eden"]),
            Err(GrantError::Invalid)
        );
        assert_eq!(
            registry.revoke_legacy_records(&["not-a-legacy-id"]),
            Err(GrantError::Invalid)
        );
        assert_eq!(fs::read(path).unwrap(), before);
    }

    #[test]
    fn legacy_grant_transaction_restores_only_changed_records_after_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("grant-authority.json");
        let records = serde_json::json!([
            {
                "version": 1,
                "persistent_grant_id": "eden-live",
                "extension_id": "eden",
                "provenance": "NativeDialog",
                "exact_file": false,
                "selected_path": dir.path().to_string_lossy(),
                "root_identity": {"primary": 1, "secondary": 2},
                "exact_file_identity": null,
                "revoked": false,
                "future_scope": {"read": ["vault"]}
            },
            {
                "version": 1,
                "persistent_grant_id": "eden-already-revoked",
                "extension_id": "eden",
                "provenance": "NativeDialog",
                "exact_file": false,
                "selected_path": dir.path().to_string_lossy(),
                "root_identity": {"primary": 3, "secondary": 4},
                "exact_file_identity": null,
                "revoked": true,
                "future_marker": "do-not-resurrect"
            },
            {
                "version": 1,
                "persistent_grant_id": "delphi-untouched",
                "extension_id": "delphi",
                "provenance": "NativeDialog",
                "exact_file": false,
                "selected_path": dir.path().to_string_lossy(),
                "root_identity": {"primary": 5, "secondary": 6},
                "exact_file_identity": null,
                "revoked": false,
                "future_marker": "unrelated"
            }
        ]);
        fs::write(&path, serde_json::to_vec(&records).unwrap()).unwrap();

        let result = GrantAuthorityRegistry::with_data_dir(dir.path().to_path_buf())
            .revoke_legacy_records_transaction(&["eden"])
            .unwrap();
        assert_eq!(result.revoked, 1);
        let token = result.transaction_token.unwrap();
        let revoked: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(revoked[0]["revoked"], true);
        assert_eq!(revoked[0]["future_scope"]["read"][0], "vault");
        assert_eq!(revoked[1]["revoked"], true);
        assert_eq!(revoked[2]["revoked"], false);

        let restarted = GrantAuthorityRegistry::with_data_dir(dir.path().to_path_buf());
        assert_eq!(
            restarted
                .restore_migration_snapshot(&["eden"], Some(&token))
                .unwrap()
                .restored,
            1
        );
        let restored: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(restored[0]["revoked"], false);
        assert_eq!(restored[0]["future_scope"]["read"][0], "vault");
        assert_eq!(restored[1]["revoked"], true);
        assert_eq!(restored[1]["future_marker"], "do-not-resurrect");
        assert_eq!(restored[2]["revoked"], false);
        assert_eq!(
            restarted
                .restore_migration_snapshot(&["eden"], Some(&token))
                .unwrap()
                .restored,
            0
        );
    }

    #[test]
    fn legacy_grant_restore_failure_is_retryable_after_restart_and_commit_is_terminal() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("grant-authority.json");
        let records = serde_json::json!([{
            "version": 1,
            "persistent_grant_id": "eden-grant",
            "extension_id": "eden",
            "provenance": "NativeDialog",
            "exact_file": false,
            "selected_path": dir.path().to_string_lossy(),
            "root_identity": {"primary": 1, "secondary": 2},
            "exact_file_identity": null,
            "revoked": false,
            "opaque": {"keep": true}
        }]);
        fs::write(&path, serde_json::to_vec(&records).unwrap()).unwrap();

        let result = GrantAuthorityRegistry::with_data_dir(dir.path().to_path_buf())
            .revoke_legacy_records_transaction(&["eden"])
            .unwrap();
        let token = result.transaction_token.unwrap();

        FAIL_PERSIST_AFTER_FSYNC.with(|fail| fail.set(true));
        assert_eq!(
            GrantAuthorityRegistry::with_data_dir(dir.path().to_path_buf())
                .restore_migration_snapshot(&["eden"], Some(&token))
                .map(|result| result.restored),
            Err(GrantError::Persistence)
        );
        FAIL_PERSIST_AFTER_FSYNC.with(|fail| fail.set(false));

        let restarted = GrantAuthorityRegistry::with_data_dir(dir.path().to_path_buf());
        assert_eq!(
            restarted
                .restore_migration_snapshot(&["eden"], Some(&token))
                .unwrap()
                .restored,
            1
        );
        assert_eq!(
            restarted.commit_legacy_records(&token),
            Err(GrantError::Invalid)
        );

        fs::write(&path, serde_json::to_vec(&records).unwrap()).unwrap();
        let committed = GrantAuthorityRegistry::with_data_dir(dir.path().to_path_buf())
            .revoke_legacy_records_transaction(&["eden"])
            .unwrap();
        let committed_token = committed.transaction_token.unwrap();
        GrantAuthorityRegistry::with_data_dir(dir.path().to_path_buf())
            .commit_legacy_records(&committed_token)
            .unwrap();
        assert_eq!(
            GrantAuthorityRegistry::with_data_dir(dir.path().to_path_buf())
                .restore_migration_snapshot(&["eden"], Some(&committed_token))
                .map(|result| result.restored),
            Err(GrantError::Invalid)
        );
        let final_records: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(final_records[0]["revoked"], true);
        assert_eq!(final_records[0]["opaque"]["keep"], true);
    }

    #[test]
    fn migration_snapshot_resolves_active_transaction_without_persisted_token() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("grant-authority.json");
        let records = serde_json::json!([{
            "version": 1,
            "persistent_grant_id": "eden-grant",
            "extension_id": "eden",
            "provenance": "NativeDialog",
            "exact_file": false,
            "selected_path": dir.path().to_string_lossy(),
            "root_identity": {"primary": 1, "secondary": 2},
            "exact_file_identity": null,
            "revoked": false
        }]);
        fs::write(&path, serde_json::to_vec(&records).unwrap()).unwrap();

        let revoked = GrantAuthorityRegistry::with_data_dir(dir.path().to_path_buf())
            .revoke_legacy_records_transaction(&["eden"])
            .unwrap();
        assert_eq!(revoked.revoked, 1);
        let restarted = GrantAuthorityRegistry::with_data_dir(dir.path().to_path_buf());
        let restored = restarted
            .restore_migration_snapshot(&["eden"], None)
            .unwrap();
        assert_eq!(
            restored,
            MigrationGrantRestoration {
                snapshot_found: true,
                restored: 1
            }
        );
    }

    #[test]
    fn stale_snapshot_token_cannot_restore_by_source_set() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("grant-authority.json");
        let records = serde_json::json!([{
            "version": 1,
            "persistent_grant_id": "eden-grant",
            "extension_id": "eden",
            "provenance": "NativeDialog",
            "exact_file": false,
            "selected_path": dir.path().to_string_lossy(),
            "root_identity": {"primary": 1, "secondary": 2},
            "exact_file_identity": null,
            "revoked": false
        }]);
        fs::write(&path, serde_json::to_vec(&records).unwrap()).unwrap();
        GrantAuthorityRegistry::with_data_dir(dir.path().to_path_buf())
            .revoke_legacy_records_transaction(&["eden"])
            .unwrap();

        let restarted = GrantAuthorityRegistry::with_data_dir(dir.path().to_path_buf());
        assert_eq!(
            restarted.restore_migration_snapshot(&["eden"], Some("stale-token")),
            Err(GrantError::NotFound)
        );
        let persisted: serde_json::Value =
            serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        assert_eq!(persisted[0]["revoked"], true);
    }
}
