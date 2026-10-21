pub(crate) async fn load_or_generate_secret_key_via_actor(
    db: &crate::db::DbHandle,
) -> Result<ark_net::iroh::SecretKey, String> {
    let candidate = ark_net::iroh::generate_secret_key_candidate();
    let persisted = db
        .submit_iroh_identity_key_candidate(crate::db::CommandContext::engine(), candidate)
        .await
        .map_err(|error| error.to_string())?;
    ark_net::iroh::parse_secret_key_hex(&persisted)
}

pub use ark_net::iroh::*;

#[cfg(test)]
#[path = "iroh_transport/bidirectional_tests.rs"]
mod bidirectional_tests;
#[cfg(test)]
#[path = "iroh_transport/stop_teardown_tests.rs"]
mod stop_teardown_tests;
