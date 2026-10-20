use super::ObjectReadCapabilities;
use ark_application::data_volume::{DataVolumeEstimate, DataVolumeQuery, DataVolumeReadPort};
use ark_application::load_all_data::LoadAllData;
use ark_application::object_read::SearchResult;
use ark_domain::object_write::ObjectWriteSnapshot;
use ark_domain::types::{ArkObject, ArkObjectSummary};

impl ObjectReadCapabilities<'_> {
    pub fn load_all(&self) -> Result<LoadAllData, String> {
        crate::load_all::load_all(self.connection)
    }

    pub fn list(&self) -> Result<Vec<ArkObject>, String> {
        crate::objects::list_objects(self.connection)
    }

    pub fn list_summaries(&self) -> Result<Vec<ArkObjectSummary>, String> {
        crate::objects::list_object_summaries(self.connection)
    }

    pub fn list_by_type(&self, type_id: &str) -> Result<Vec<ArkObject>, String> {
        crate::objects::list_objects_by_type(self.connection, type_id)
    }

    pub fn list_summaries_by_type(&self, type_id: &str) -> Result<Vec<ArkObjectSummary>, String> {
        crate::objects::list_object_summaries_by_type(self.connection, type_id)
    }

    pub fn estimate_data_volume(
        &self,
        query: &DataVolumeQuery,
    ) -> Result<Vec<DataVolumeEstimate>, String> {
        let session = crate::data_volume::SqliteDataVolumeReadSession::new(self.connection);
        session.estimate_data_volume(query)
    }

    pub fn list_running_time(&self, source: Option<&str>) -> Result<Vec<ArkObject>, String> {
        crate::objects::list_running_time_entries(self.connection, source)
    }

    pub fn get_by_ids(&self, ids: &[String]) -> Result<Vec<ArkObject>, String> {
        crate::objects::get_objects_by_ids(self.connection, ids)
    }

    pub fn search(&self, query: &str) -> Result<Vec<SearchResult>, String> {
        crate::objects::search_objects(self.connection, query)
    }

    pub fn get(&self, id: &str) -> Result<Option<ArkObject>, String> {
        crate::objects::get_object(self.connection, id)
    }

    pub fn write_snapshot(&self, id: &str) -> Result<ObjectWriteSnapshot, String> {
        crate::object_write::read_object_write_snapshot(self.connection, id)
    }
}
