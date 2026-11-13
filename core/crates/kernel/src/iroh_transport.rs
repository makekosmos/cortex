pub(crate) async fn load_or_generate_secret_key_via_actor(
    db: &crate::db::DbHandle,
) -> Result<::net::iroh::SecretKey, String> {
    let candidate = ::net::iroh::generate_secret_key_candidate();
    let persisted = db
        .submit_iroh_identity_key_candidate(crate::db::CommandContext::engine(), candidate)
        .await
        .map_err(|error| error.to_string())?;
    ::net::iroh::parse_secret_key_hex(&persisted)
}

pub use ::net::iroh::*;
