//! Typed Arrancador Game facade. Generic RPC and persisted representation stay in core.

use crate::ark_host::ArkHost;
use ark_core::canonical_types::game::{
    GameLocalState, GameQuarantine, GameRecord, GameUpsertCommand, GameUsage,
};
use serde_json::{json, Value};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ArrancadorGameProjection {
    pub game: GameRecord,
    pub local: GameLocalState,
    pub quarantine: GameQuarantine,
    pub usage: GameUsage,
}

pub struct GameFacade<'a> {
    ark: &'a ArkHost,
}

impl<'a> GameFacade<'a> {
    pub fn new(ark: &'a ArkHost) -> Self {
        Self { ark }
    }

    pub async fn list(&self) -> Result<Value, String> {
        let games = self
            .ark
            .canonical_game_list()
            .await
            .map_err(|e| e.to_string())?;
        serde_json::to_value(games.into_iter().map(Self::projection).collect::<Vec<_>>())
            .map_err(|e| e.to_string())
    }

    pub async fn read(&self, id: &str) -> Result<Value, String> {
        let game = self.object(id).await?;
        serde_json::to_value(Self::projection(game)).map_err(|e| e.to_string())
    }

    pub async fn mutation_record(&self, id: &str) -> Result<GameRecord, String> {
        self.object(id).await
    }

    pub async fn upsert(&self, command: GameUpsertCommand) -> Result<GameRecord, String> {
        self.ark
            .canonical_game_upsert(command)
            .await
            .map(|result| result.record)
            .map_err(|e| e.to_string())
    }

    pub async fn objects(&self) -> Result<Vec<GameRecord>, String> {
        self.ark
            .canonical_game_list()
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn object(&self, id: &str) -> Result<GameRecord, String> {
        self.ark
            .canonical_game_get(id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "game_not_found".into())
    }

    pub fn launch_dto(game: &GameRecord) -> super::launcher::LaunchGame {
        super::launcher::LaunchGame {
            id: game.id.clone(),
            title: game.title.clone(),
        }
    }

    pub fn sqoba_metadata(game: &GameRecord) -> (String, Option<Vec<std::path::PathBuf>>) {
        (game.title.clone(), None)
    }

    fn projection(game: GameRecord) -> ArrancadorGameProjection {
        ArrancadorGameProjection {
            local: game.local.clone(),
            quarantine: game.quarantine.clone(),
            usage: game.usage.clone(),
            game,
        }
    }

    pub fn upsert_payload(
        existing: Option<&GameRecord>,
        id: &str,
        title: &str,
        patch: &Value,
        now: &str,
    ) -> GameUpsertCommand {
        let mut command = existing
            .cloned()
            .map(|game| GameUpsertCommand {
                id: game.id,
                title: game.title,
                play_status: game.play_status,
                user_rating: game.user_rating,
                genres: game.genres,
                platforms: game.platforms,
                released: game.released,
                description: game.description,
                extensions: game.extensions,
                created_at: game.created_at,
                updated_at: now.into(),
                deleted_at: game.deleted_at,
                links: game.links,
                local: game.local,
                quarantine: game.quarantine,
                device_id: None,
            })
            .unwrap_or_else(|| {
                Self::new_command(
                    id.into(),
                    title.into(),
                    now.into(),
                    Default::default(),
                    Default::default(),
                )
            });
        command.id = id.into();
        command.title = title.into();
        command.updated_at = now.into();
        if let Some(value) = patch.get("playStatus").and_then(Value::as_str) {
            command.play_status = Some(value.into());
        }
        if let Some(values) = patch.get("platforms").and_then(Value::as_array) {
            command.platforms = values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect();
        }
        command
    }

    pub fn new_command(
        id: String,
        title: String,
        now: String,
        local: GameLocalState,
        quarantine: GameQuarantine,
    ) -> GameUpsertCommand {
        GameUpsertCommand {
            id,
            title,
            play_status: Some("notStarted".into()),
            user_rating: None,
            genres: vec![],
            platforms: vec![],
            released: None,
            description: None,
            extensions: json!({}),
            created_at: now.clone(),
            updated_at: now,
            deleted_at: None,
            links: vec![],
            local,
            quarantine,
            device_id: None,
        }
    }
}
