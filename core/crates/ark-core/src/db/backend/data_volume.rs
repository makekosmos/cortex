use ark_application::data_volume::{DataVolumeEstimate, DataVolumeQuery, DataVolumeReadPort};

pub(crate) struct SqliteDataVolumeReadSession<'connection> {
    connection: &'connection rusqlite::Connection,
}

impl<'connection> SqliteDataVolumeReadSession<'connection> {
    pub(crate) fn new(connection: &'connection rusqlite::Connection) -> Self {
        Self { connection }
    }
}

impl DataVolumeReadPort for SqliteDataVolumeReadSession<'_> {
    fn estimate_data_volume(
        &self,
        query: &DataVolumeQuery,
    ) -> Result<Vec<DataVolumeEstimate>, String> {
        crate::db::estimate_data_volume(self.connection, query)
    }
}
