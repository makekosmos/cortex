use super::ffi_legacy::{
    ffi_delete_planning_object, ffi_project_record, ffi_tag_record, ffi_todo_record,
    ffi_write_legacy_records,
};
use super::*;

#[uniffi::export]
impl ArkCore {
    /// Construct a brand-new ArkCore facade. The DB is **not** opened yet —
    /// call `open_db(path)` before issuing any CRUD method.
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("ark-core-ffi")
            .build()
            .expect("build tokio runtime");
        Arc::new(Self {
            runtime,
            db: StdMutex::new(None),
            sync_shutdown: StdMutex::new(None),
            sync: TokioMutex::new(None),
            listener: RwLock::new(None),
        })
    }

    // -----------------------------------------------------------------------
    // DB lifecycle + CRUD
    // -----------------------------------------------------------------------

    pub fn open_db(&self, path: String) -> Result<bool> {
        let conn = open_db(&path).map_err(ArkCoreError::from)?;
        init_schema(&conn).map_err(ArkCoreError::from)?;
        *self.db.lock().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(StdMutex::new(conn)));
        Ok(true)
    }

    pub fn load_all_json(&self) -> Result<String> {
        self.with_conn(|conn| {
            let data: LoadAllData = db_load_all(conn).map_err(ArkCoreError::from)?;
            serde_json::to_string(&data).map_err(|e| err(e.to_string()))
        })
    }

    pub fn upsert_todo_json(&self, todo_json: String) -> Result<bool> {
        let todo: TodoItem =
            serde_json::from_str(&todo_json).map_err(|_| malformed_json_error("/todo"))?;
        self.with_conn(|conn| {
            ffi_write_legacy_records(conn, vec![ffi_todo_record(&todo)])
                .map(|_| true)
                .map_err(compatibility_error)
        })
    }

    pub fn delete_todo(&self, id: String) -> Result<bool> {
        self.with_conn(|conn| {
            ffi_delete_planning_object(conn, &id, "com.kosmos.task")
                .map(|_| true)
                .map_err(compatibility_error)
        })
    }

    pub fn batch_upsert_todos_json(&self, todos_json: String) -> Result<bool> {
        let todos: Vec<TodoItem> =
            serde_json::from_str(&todos_json).map_err(|_| malformed_json_error("/todos"))?;
        self.with_conn(|conn| {
            ffi_write_legacy_records(conn, todos.iter().map(ffi_todo_record).collect())
                .map(|_| true)
                .map_err(compatibility_error)
        })
    }

    pub fn upsert_project_json(&self, project_json: String) -> Result<bool> {
        let project: Project =
            serde_json::from_str(&project_json).map_err(|_| malformed_json_error("/project"))?;
        self.with_conn(|conn| {
            ffi_write_legacy_records(conn, vec![ffi_project_record(&project)])
                .map(|_| true)
                .map_err(compatibility_error)
        })
    }

    pub fn delete_project(&self, id: String) -> Result<bool> {
        self.with_conn(|conn| {
            ffi_delete_planning_object(conn, &id, "com.kosmos.project")
                .map(|_| true)
                .map_err(compatibility_error)
        })
    }

    pub fn upsert_area_json(&self, _area_json: String) -> Result<bool> {
        Err(structured_error(
            "compatibility",
            "LEGACY_PLANNING_READ_ONLY",
            None,
        ))
    }

    pub fn upsert_tag_json(&self, tag_json: String) -> Result<bool> {
        let tag: Tag = serde_json::from_str(&tag_json).map_err(|_| malformed_json_error("/tag"))?;
        self.with_conn(|conn| {
            ffi_write_legacy_records(conn, vec![ffi_tag_record(&tag)])
                .map(|_| true)
                .map_err(compatibility_error)
        })
    }

    pub fn upsert_heading_json(&self, _heading_json: String) -> Result<bool> {
        Err(structured_error(
            "compatibility",
            "LEGACY_PLANNING_READ_ONLY",
            None,
        ))
    }

    pub fn delete_heading(&self, _id: String) -> Result<bool> {
        Err(structured_error(
            "compatibility",
            "LEGACY_PLANNING_READ_ONLY",
            None,
        ))
    }

    pub fn upsert_tracked_app_json(&self, tracked_app_json: String) -> Result<bool> {
        let tracked_app: TrackedApp = serde_json::from_str(&tracked_app_json)
            .map_err(|_| malformed_json_error("/trackedApp"))?;
        self.with_conn(|conn| {
            db_upsert_tracked_app(conn, &tracked_app).map_err(ArkCoreError::from)?;
            Ok(true)
        })
    }

    pub fn delete_tracked_app(&self, id: String) -> Result<bool> {
        self.with_conn(|conn| {
            db_delete_tracked_app(conn, &id).map_err(ArkCoreError::from)?;
            Ok(true)
        })
    }

    pub fn upsert_usage_session_json(&self, usage_session_json: String) -> Result<bool> {
        let usage_session: UsageSession = serde_json::from_str(&usage_session_json)
            .map_err(|_| malformed_json_error("/usageSession"))?;
        self.with_conn(|conn| {
            db_upsert_usage_session(conn, &usage_session).map_err(ArkCoreError::from)?;
            Ok(true)
        })
    }

    pub fn delete_usage_session(&self, id: String) -> Result<bool> {
        self.with_conn(|conn| {
            db_delete_usage_session(conn, &id).map_err(ArkCoreError::from)?;
            Ok(true)
        })
    }

    pub fn upsert_usage_event_json(&self, usage_event_json: String) -> Result<bool> {
        let usage_event: UsageEvent = serde_json::from_str(&usage_event_json)
            .map_err(|_| malformed_json_error("/usageEvent"))?;
        self.with_conn(|conn| {
            db_upsert_usage_event(conn, &usage_event).map_err(ArkCoreError::from)?;
            Ok(true)
        })
    }

    pub fn delete_usage_event(&self, id: String) -> Result<bool> {
        self.with_conn(|conn| {
            db_delete_usage_event(conn, &id).map_err(ArkCoreError::from)?;
            Ok(true)
        })
    }

    pub fn get_sync_kv(&self, key: String) -> Result<Option<String>> {
        self.with_conn(|conn| db_get_kv(conn, &key).map_err(ArkCoreError::from))
    }

    pub fn set_sync_kv(&self, key: String, value: String) -> Result<bool> {
        self.with_conn(|conn| {
            db_set_kv(conn, &key, &value).map_err(ArkCoreError::from)?;
            Ok(true)
        })
    }

    pub fn clear_all(&self) -> Result<bool> {
        self.with_conn(|conn| {
            db_clear_all(conn).map_err(ArkCoreError::from)?;
            Ok(true)
        })
    }

    pub fn delete_trashed(&self) -> Result<u32> {
        self.with_conn(|conn| {
            // Tombstones are stamped under the FFI surface's device label so
            // peers can attribute the delete (mirrors "ark-core-ffi" used by
            // the canonical facades for FFI legacy writes).
            let n = db_delete_trashed(conn, "ark-core-ffi").map_err(ArkCoreError::from)?;
            Ok(n as u32)
        })
    }

    // -----------------------------------------------------------------------
    // Host helpers
    // -----------------------------------------------------------------------

    pub fn host_device_name(&self) -> String {
        get_host_device_name()
    }

    pub fn own_addresses(&self, port: u32) -> Vec<String> {
        get_own_addresses(port as u16)
    }
}
