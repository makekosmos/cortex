use super::*;
pub(in crate::ws_server) async fn handle_export_op(
    subop: &str,
    params: serde_json::Value,
    ark_host: &ArkHost,
) -> LocalResponse {
    match subop {
        "list" => {
            let converters = crate::export::list_converters();
            LocalResponse::ok(serde_json::json!({ "converters": converters }))
        }
        "run" => {
            let converter_id = match params.get("converter_id").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => return LocalResponse::err("export.run: missing 'converter_id'"),
            };
            let dest_dir = match params.get("dest_dir").and_then(|v| v.as_str()) {
                Some(s) => std::path::PathBuf::from(s),
                None => return LocalResponse::err("export.run: missing 'dest_dir'"),
            };
            let converter = match crate::export::find_converter(&converter_id) {
                Some(c) => c,
                None => {
                    return LocalResponse::err(format!(
                        "export.run: unknown converter '{converter_id}'"
                    ));
                }
            };
            let format = params
                .get("format")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| converter.default_format().to_string());
            if !converter.supported_formats().contains(&format.as_str()) {
                return LocalResponse::err(format!(
                    "export.run: format '{format}' not supported by '{converter_id}'"
                ));
            }

            // Ensure dest_dir exists.
            if let Err(e) = std::fs::create_dir_all(&dest_dir) {
                return LocalResponse::err(format!("export.run: create dest_dir: {e}"));
            }

            // Fetch objects of converter's object_type через ark_host.
            let ark_resp = match ark_host
                .request(
                    "list_objects_by_type",
                    serde_json::json!({ "type_id": converter.object_type() }),
                )
                .await
            {
                Ok(r) => r,
                Err(e) => return LocalResponse::err(format!("export.run: ark_host: {e}")),
            };
            if !ark_resp.ok {
                return LocalResponse::err(format!(
                    "export.run: ark list_objects_by_type failed: {}",
                    ark_resp.error.unwrap_or_default()
                ));
            }
            let objects: Vec<ark_core::types::ArkObject> =
                match serde_json::from_value(ark_resp.data) {
                    Ok(v) => v,
                    Err(e) => {
                        return LocalResponse::err(format!(
                            "export.run: parse ArkObject array: {e}"
                        ));
                    }
                };

            let links_resp = match ark_host
                .request("list_object_links", serde_json::json!({}))
                .await
            {
                Ok(r) => r,
                Err(e) => return LocalResponse::err(format!("export.run: ark_host links: {e}")),
            };
            if !links_resp.ok {
                return LocalResponse::err(format!(
                    "export.run: ark list_object_links failed: {}",
                    links_resp.error.unwrap_or_default()
                ));
            }
            let links: Vec<ark_core::types::ObjectLink> =
                match serde_json::from_value(links_resp.data) {
                    Ok(v) => v,
                    Err(e) => {
                        return LocalResponse::err(format!(
                            "export.run: parse ObjectLink array: {e}"
                        ));
                    }
                };
            let envelopes = objects
                .into_iter()
                .map(|object| crate::export::CanonicalEnvelope {
                    links: links
                        .iter()
                        .filter(|link| link.source_object_id == object.id)
                        .cloned()
                        .collect(),
                    object,
                })
                .collect::<Vec<_>>();
            let result = converter.convert_canonical(&envelopes, &format, &dest_dir);
            match serde_json::to_value(&result) {
                Ok(v) => LocalResponse::ok(v),
                Err(e) => LocalResponse::err(format!("export.run: serialize result: {e}")),
            }
        }
        other => LocalResponse::err(format!("export.{other}: unknown sub-operation")),
    }
}
