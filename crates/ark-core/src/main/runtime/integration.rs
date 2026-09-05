use super::*;

pub(super) async fn handle(request: Request) -> Result<Value, String> {
    match request {
        Request::IntegrationPersistNodeAuthorization {
            authorization_operation,
            node,
            grant,
            device_id,
        } => with_conn(|conn| {
            let operation = parse_authorization_operation(&authorization_operation)?;
            ark_core::integration_replication::persist_node_authorization(
                conn,
                operation,
                &node,
                grant.as_ref(),
                &device_id,
            )?;
            Ok(json!(true))
        }),
        Request::IntegrationPersistIntegrationGrant { grant, device_id } => with_conn(|conn| {
            ark_core::integration_replication::persist_integration_grant(conn, &grant, &device_id)?;
            Ok(json!(true))
        }),
        Request::IntegrationPrepareSignedSync {
            space_id,
            origin_node_id,
            integration_id,
            recipient_node_id,
            message_id,
        } => with_conn(|conn| {
            let preparation = ark_core::integration_replication::prepare_signed_sync(
                conn,
                &space_id,
                &origin_node_id,
                &integration_id,
                &recipient_node_id,
                &message_id,
            )?;
            serde_json::to_value(preparation).map_err(|error| error.to_string())
        }),
        Request::IntegrationValidateOutboundSignedSync {
            space_id,
            origin_node_id,
            frame,
        } => with_conn(|conn| {
            ark_core::integration_replication::validate_outbound_signed_sync(
                conn,
                &space_id,
                &origin_node_id,
                &frame,
            )?;
            Ok(json!(true))
        }),
        _ => Err("request is not an integration replication operation".into()),
    }
}

fn parse_authorization_operation(value: &str) -> Result<ark_core::integration_replication::NodeAuthorizationOperation, String> {
    match value {
        "authorize" => Ok(ark_core::integration_replication::NodeAuthorizationOperation::Authorize),
        "revoke" => Ok(ark_core::integration_replication::NodeAuthorizationOperation::Revoke),
        "rotate" => Ok(ark_core::integration_replication::NodeAuthorizationOperation::Rotate),
        _ => Err("authorization_operation must be authorize, revoke, or rotate".into()),
    }
}
