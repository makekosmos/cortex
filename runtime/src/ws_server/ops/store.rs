use super::*;

pub(in crate::ws_server) async fn handle_store_op(
    subop: &str,
    params: serde_json::Value,
    packages: &PackageService,
) -> LocalResponse {
    if subop == "external_url" {
        let listing_id = params.get("listing_id").and_then(Value::as_str);
        return match listing_id.and_then(|id| packages.store_external_url(id).ok()) {
            Some(url) => LocalResponse::ok(serde_json::json!({ "url": url })),
            None => LocalResponse::err("store: unavailable"),
        };
    }
    let installed = match packages.store_installed_listings() {
        Ok(installed) => installed,
        Err(_) => return LocalResponse::err("store: installed-packages-unavailable"),
    };
    let response = match subop {
        "catalog" => Ok(packages.store_catalog(chrono::Utc::now(), installed)),
        // The one catalog is both the storefront and the install authority —
        // a single refresh serves both.
        "refresh" => match packages.refresh_catalog().await {
            Ok(_) => Ok(packages.store_catalog(chrono::Utc::now(), installed)),
            Err(_) => Err("store: refresh-unavailable"),
        },
        other => return LocalResponse::err(format!("store.{other}: unknown sub-operation")),
    };
    match response
        .and_then(|result| serde_json::to_value(result).map_err(|_| "store: serialization-failed"))
    {
        Ok(value) => LocalResponse::ok(value),
        Err(error) => LocalResponse::err(error),
    }
}
