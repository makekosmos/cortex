use std::path::Path;

/// Seeds the records used by Core's data-volume privacy and sizing test.
pub struct DataVolumeFixture;

impl DataVolumeFixture {
    pub fn seed(path: &Path) -> Result<(), String> {
        let path = path
            .to_str()
            .ok_or_else(|| "data volume database path is not UTF-8".to_string())?;
        let connection = crate::open::open_db(path)?;
        crate::open::init_schema(&connection)?;
        connection
            .execute_batch(
                "INSERT INTO object_types
                 (id,name,schema_json,ui_schema_json,created_at,updated_at,system_locked)
                 VALUES ('estimate_type','Estimate','{}','{}',
                 '2026-01-01T00:00:00.000Z','2026-01-01T00:00:00.000Z',0);
                 INSERT INTO objects (id,type_id,type_version,title,content_json,props_json,
                    created_at,updated_at,deleted_at)
                 VALUES ('estimate-1','estimate_type','1.0.0','Visible title',
                 '{\"text\":\"private body\"}', '{\"allowed\":\"yes\"}',
                 '2026-01-01T00:00:00.000Z','2026-01-02T00:00:00.000Z',NULL);",
            )
            .map_err(|error| error.to_string())
    }
}
