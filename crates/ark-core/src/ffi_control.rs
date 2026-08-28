use super::*;

#[uniffi::export]
impl ArkCore {
    pub fn stop_sync(&self) -> Result<bool> {
        // Signal the background sync thread to exit.
        if let Some(tx) = self
            .sync_shutdown
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
        {
            let _ = tx.send(());
        }
        self.runtime.block_on(async {
            self.stop_sync_inner().await;
            Ok(true)
        })
    }

    pub fn leave_space(&self) -> Result<bool> {
        self.stop_sync()
    }

    /// Step 4a: our iroh pairing ticket, if the running sync selected the
    /// iroh transport. `None` when sync isn't running, relay/no transport
    /// was selected instead, or this build lacks `iroh-spike`.
    pub fn get_own_iroh_ticket(&self) -> Result<Option<String>> {
        self.runtime.block_on(async {
            let guard = self.sync.lock().await;
            Ok(guard.as_ref().and_then(|r| r.iroh_our_ticket.clone()))
        })
    }

    /// Typed canonical Image/rich-text source projection. The JSON string is
    /// deliberately the only FFI wire type so bindings remain stable.
    pub fn canonical_asset_sources_json(&self, object_ids_json: String) -> Result<String> {
        let object_ids: Vec<String> = serde_json::from_str(&object_ids_json).map_err(|_| {
            structured_error("invalid_request", "malformed_json", Some("/objectIds"))
        })?;
        self.with_conn(|conn| {
            let sources = crate::canonical_types::facades::asset_sources(conn, &object_ids)
                .map_err(|e| structured_error("canonical", e.code, e.pointer.as_deref()))?;
            serde_json::to_string(&sources)
                .map_err(|_| structured_error("storage", "serialize", None))
        })
    }

    /// Typed canonical Book cover command. No legacy `cover_image` property
    /// is read or written by this boundary.
    pub fn canonical_set_book_cover(
        &self,
        book_id: String,
        source_ref: Option<String>,
        existing_image_id: Option<String>,
        alt_text: String,
    ) -> Result<bool> {
        self.with_conn(|conn| {
            conn.execute_batch("SAVEPOINT ffi_canonical_cover")
                .map_err(|e| err(e.to_string()))?;
            let result = (|| {
                let mutation = crate::canonical_types::facades::set_book_cover(
                    conn,
                    &book_id,
                    source_ref.as_deref(),
                    existing_image_id.as_deref(),
                    &alt_text,
                    "ffi-local",
                )
                .map_err(|e| structured_error("canonical", e.code, e.pointer.as_deref()))?;
                if mutation.changed {
                    let object_hlc = crate::db::bump_sync_version_vector(
                        conn,
                        "object",
                        &book_id,
                        "ffi-local",
                        false,
                    )?;
                    conn.execute(
                        "INSERT INTO object_sync_versions(object_id,hlc,deleted) VALUES(?1,?2,0) ON CONFLICT(object_id) DO UPDATE SET hlc=excluded.hlc,deleted=0 WHERE excluded.hlc > object_sync_versions.hlc",
                        rusqlite::params![book_id, object_hlc],
                    )
                    .map_err(|e| e.to_string())?;
                    for link in &mutation.links {
                        let link_hlc = crate::db::bump_sync_version_vector(
                            conn,
                            "object_link",
                            &link.id,
                            "ffi-local",
                            false,
                        )?;
                        conn.execute(
                            "INSERT INTO object_sync_versions(object_id,hlc,deleted) VALUES(?1,?2,0) ON CONFLICT(object_id) DO UPDATE SET hlc=excluded.hlc,deleted=0 WHERE excluded.hlc > object_sync_versions.hlc",
                            rusqlite::params![link.id, link_hlc],
                        )
                        .map_err(|e| e.to_string())?;
                    }
                    for link_id in &mutation.deleted_link_ids {
                        crate::db::bump_sync_version_vector(
                            conn,
                            "object_link",
                            link_id,
                            "ffi-local",
                            true,
                        )?;
                    }
                }
                Ok(mutation.changed)
            })();
            match result {
                Ok(changed) => {
                    conn.execute_batch("RELEASE ffi_canonical_cover")
                        .map_err(|e| err(e.to_string()))?;
                    Ok(changed)
                }
                Err(error) => {
                    let _ = conn.execute_batch(
                        "ROLLBACK TO ffi_canonical_cover; RELEASE ffi_canonical_cover",
                    );
                    Err(error)
                }
            }
        })
    }
}
