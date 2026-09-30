use super::*;

pub fn refresh_transport_public_keys_named(
    setup: &IntegrationReplicationSetup,
    origin_ticket: &str,
    recipient_ticket: &str,
    integration_id: &str,
) -> Result<(), String> {
    let origin_transport_key = endpoint_id_from_ticket(origin_ticket)?;
    let recipient_transport_key = endpoint_id_from_ticket(recipient_ticket)?;
    let origin_conn = Connection::open(&setup.origin_db).map_err(|error| error.to_string())?;
    rotate_node(
        &origin_conn,
        &setup.origin,
        &origin_transport_key,
        &setup.origin.node_id,
        integration_id,
    )?;
    rotate_node(
        &origin_conn,
        &setup.recipient,
        &recipient_transport_key,
        &setup.origin.node_id,
        integration_id,
    )?;
    let recipient_conn =
        Connection::open(&setup.recipient_db).map_err(|error| error.to_string())?;
    rotate_node(
        &recipient_conn,
        &setup.origin,
        &origin_transport_key,
        &setup.recipient.node_id,
        integration_id,
    )?;
    rotate_node(
        &recipient_conn,
        &setup.recipient,
        &recipient_transport_key,
        &setup.recipient.node_id,
        integration_id,
    )?;
    Ok(())
}

fn rotate_node(
    conn: &Connection,
    node: &NodeIdentity,
    transport_key: &str,
    writer: &str,
    integration_id: &str,
) -> Result<(), String> {
    let mut authorized = authorized_node(node);
    authorized.transport_public_key = Some(transport_key.to_owned());
    authorized.grant_epoch = 2;
    authorized.revision = 2;
    authorized.hlc = format!("{HLC_WALL}:000003:{}", node.node_id);
    upsert_authorized_node(conn, &authorized, writer)?;
    upsert_integration_node_grant(conn, &transport_grant(node, integration_id), writer)
}

fn transport_grant(node: &NodeIdentity, integration_id: &str) -> IntegrationNodeGrant {
    IntegrationNodeGrant {
        integration_id: integration_id.into(),
        node_id: node.node_id.clone(),
        node_encryption_key: node.encryption_public_key.clone(),
        grant_epoch: 2,
        status: GrantStatus::Active,
        authorized_at: "2026-09-06T00:00:00Z".into(),
        revoked_at: None,
        revision: 2,
        hlc: format!("{HLC_WALL}:000003:{}", node.node_id),
    }
}

fn endpoint_id_from_ticket(ticket: &str) -> Result<String, String> {
    let encoded = ticket
        .strip_prefix("endpoint")
        .ok_or_else(|| "Iroh ticket must use the endpoint prefix".to_string())?;
    let mut buffer = 0u32;
    let mut bits = 0u8;
    let mut bytes = Vec::new();
    for value in encoded.bytes().map(|byte| byte.to_ascii_uppercase()) {
        let value = match value {
            b'A'..=b'Z' => value - b'A',
            b'2'..=b'7' => value - b'2' + 26,
            _ => return Err("Iroh ticket contains invalid base32".to_string()),
        };
        buffer = (buffer << 5) | u32::from(value);
        bits += 5;
        while bits >= 8 {
            bits -= 8;
            bytes.push(((buffer >> bits) & 0xff) as u8);
            if bits == 0 {
                buffer = 0;
            } else {
                buffer &= (1 << bits) - 1;
            }
        }
    }
    if bytes.len() < 33 || bytes[0] != 0 {
        return Err("Iroh ticket has an unsupported endpoint encoding".to_string());
    }
    Ok(hex(&bytes[1..33]))
}
