use super::*;

pub(crate) async fn dispatch_typed_inner(
    inner: &SupervisorInner,
    id: &str,
    version: &str,
    session_id: &str,
    generation: u64,
    request: serde_json::Value,
) -> Result<serde_json::Value, &'static str> {
    let launch = lock(&inner.typed_launches)
        .get(&(id.to_owned(), version.to_owned()))
        .cloned()
        .ok_or("forbidden")?;
    if launch.session_id != session_id {
        return Err("forbidden");
    }
    if launch.generation != generation {
        return Err("stale-generation");
    }
    let request = serde_json::from_value::<DataRequest>(request).map_err(|_| "invalid-request")?;
    launch
        .grant
        .authorize_request(&request)
        .map_err(|_| "forbidden")?;
    let params = serde_json::to_value(&request).map_err(|_| "invalid-request")?;
    inner.ark_executor.request("data.request", params).await
}
