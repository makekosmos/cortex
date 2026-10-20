use ark_domain::integration_replication::{
    integration_node_id, validate_authorized_node_revision, validate_configuration_revision,
    validate_integration_grant_revision, AuthorizedNode, IntegrationConfiguration,
    IntegrationNodeGrant,
};
use rusqlite::Connection;

use super::admin::persist_with_vector;
use super::{
    load_authorized_node, load_integration_configuration, load_integration_node_grant,
    upsert_authorized_node_record, upsert_integration_configuration_record,
    upsert_integration_node_grant_record,
};

pub fn upsert_integration_configuration(
    connection: &Connection,
    configuration: &IntegrationConfiguration,
    device_id: &str,
) -> Result<(), String> {
    persist_with_vector(
        connection,
        "integration_configuration",
        &configuration.integration_id,
        device_id,
        || {
            let current =
                load_integration_configuration(connection, &configuration.integration_id)?;
            validate_configuration_revision(current.as_ref(), configuration)
                .map_err(|error| error.to_string())?;
            upsert_integration_configuration_record(connection, configuration)
        },
    )
}

pub fn upsert_authorized_node(
    connection: &Connection,
    node: &AuthorizedNode,
    device_id: &str,
) -> Result<(), String> {
    persist_with_vector(
        connection,
        "authorized_node",
        &node.node_id,
        device_id,
        || {
            let current = load_authorized_node(connection, &node.node_id)?;
            validate_authorized_node_revision(current.as_ref(), node)
                .map_err(|error| error.to_string())?;
            upsert_authorized_node_record(connection, node)
        },
    )
}

pub fn upsert_integration_node_grant(
    connection: &Connection,
    grant: &IntegrationNodeGrant,
    device_id: &str,
) -> Result<(), String> {
    let entity_id = integration_node_id(&grant.integration_id, &grant.node_id);
    persist_with_vector(
        connection,
        "integration_node_grant",
        &entity_id,
        device_id,
        || {
            let current =
                load_integration_node_grant(connection, &grant.integration_id, &grant.node_id)?;
            let node = load_authorized_node(connection, &grant.node_id)?;
            validate_integration_grant_revision(current.as_ref(), node.as_ref(), grant)
                .map_err(|error| error.to_string())?;
            upsert_integration_node_grant_record(connection, grant)
        },
    )
}
