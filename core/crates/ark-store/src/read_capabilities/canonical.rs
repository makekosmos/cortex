use super::CanonicalReadCapabilities;
use crate::canonical_asset_sources::{AssetSource, AssetSourceError};
use ark_application::canonical_game::GameRecord;

impl CanonicalReadCapabilities<'_> {
    pub fn games(&self, device_id: &str) -> Result<Vec<GameRecord>, String> {
        crate::canonical_game::list_games(self.connection, device_id)
    }

    pub fn game(&self, id: &str, device_id: &str) -> Result<GameRecord, String> {
        crate::canonical_game::get_game(self.connection, id, device_id)
    }

    pub fn asset_sources(
        &self,
        object_ids: &[String],
    ) -> Result<Vec<AssetSource>, AssetSourceError> {
        crate::canonical_asset_sources::asset_sources(self.connection, object_ids)
    }
}
