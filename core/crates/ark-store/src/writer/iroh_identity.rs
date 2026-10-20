use super::{Connection, SqliteWriter, StoreTransactionFailure, WriteCapabilities};

const IROH_SECRET_KEY_KV_KEY: &str = "iroh.secret_key";

pub struct IrohIdentityWrites<'scope, 'connection> {
    transaction: &'scope rusqlite::Transaction<'connection>,
}

impl WriteCapabilities<'_, '_> {
    pub fn iroh_identity(&mut self) -> IrohIdentityWrites<'_, '_> {
        IrohIdentityWrites {
            transaction: self.transaction,
        }
    }
}

impl IrohIdentityWrites<'_, '_> {
    pub fn persist_candidate(&mut self, candidate_hex: &str) -> Result<String, String> {
        crate::sync_kv::get_or_insert_sync_kv(
            self.transaction,
            IROH_SECRET_KEY_KV_KEY,
            candidate_hex,
        )
    }
}

impl SqliteWriter {
    pub fn persist_iroh_secret_key_candidate(
        &mut self,
        candidate_hex: String,
    ) -> Result<String, StoreTransactionFailure<String>> {
        self.write_with_capabilities(|capabilities| {
            capabilities
                .iroh_identity()
                .persist_candidate(&candidate_hex)
        })
    }

    pub fn persist_iroh_secret_key_candidate_on_connection(
        connection: &mut Connection,
        candidate_hex: String,
    ) -> Result<String, StoreTransactionFailure<String>> {
        Self::write_with_capabilities_on_connection(connection, |capabilities| {
            capabilities
                .iroh_identity()
                .persist_candidate(&candidate_hex)
        })
    }
}
