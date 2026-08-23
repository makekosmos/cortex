use super::*;
pub(in crate::ws_server) async fn handle_calculator_op(
    subop: &str,
    params: serde_json::Value,
    data_dir: &std::path::Path,
) -> LocalResponse {
    match subop {
        "evaluate" => {
            let query = match params.get("query").and_then(|value| value.as_str()) {
                Some(query) => query.to_string(),
                None => return LocalResponse::err("calculator.evaluate: missing 'query'"),
            };
            let normalized = crate::calculator::normalize_query(&query);
            let rates =
                crate::calculator::exchange_rates_for_query(&normalized.evaluation, data_dir).await;
            let expression = normalized.display_expression;
            let evaluation = normalized.evaluation;
            match tokio::task::spawn_blocking(move || {
                crate::calculator::evaluate_preview_with_rates(&evaluation, rates)
            })
            .await
            {
                Ok(result) => LocalResponse::ok(serde_json::json!({
                    "result": result,
                    "expression": expression,
                })),
                Err(error) => LocalResponse::err(format!("calculator.evaluate: join: {error}")),
            }
        }
        other => LocalResponse::err(format!("calculator.{other}: unknown sub-operation")),
    }
}
