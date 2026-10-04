use super::*;

impl GrantAuthorityRegistry {
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
    /// Run `operation` against the root handle of an authorized directory
    /// grant. Owner, extension and root-identity checks are identical to the
    /// per-file entry points; `exact_file` grants are always rejected here so
    /// vault-style operations cannot treat a file grant as a directory.
    pub fn with_directory_root<T>(
        &self,
        grant_id: &str,
        owner: &GrantOwner,
        extension_id: &str,
        operation: impl FnOnce(&RootHandle) -> io::Result<T>,
    ) -> Result<T, GrantError> {
        self.with_authorized_grant(grant_id, owner, extension_id, &["."], false, |grant| {
            operation(&grant.root).map_err(|_| GrantError::ScopeMismatch)
        })
    }
    /// Live grant count for one owner — used to bound root-handle retention
    /// for `/v1/rpc` vault roots, which have no disconnect hook.
    pub fn owner_grant_count(&self, owner: &GrantOwner) -> usize {
        self.grants
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .values()
            .filter(|g| &g.owner == owner)
            .count()
    }
    /// Drop one grant owned by `owner`. Returns false when the id is unknown
    /// or belongs to a different owner (never removes across owners).
    pub fn close(&self, grant_id: &str, owner: &GrantOwner) -> bool {
        let mut grants = self.grants.lock().unwrap_or_else(|p| p.into_inner());
        if grants
            .get(grant_id)
            .is_some_and(|grant| &grant.owner == owner)
        {
            grants.remove(grant_id);
            return true;
        }
        false
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
}
