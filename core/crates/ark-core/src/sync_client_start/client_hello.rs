use crate::protocol::{
    compute_hello_auth_hmac, compute_negotiation_offer_hmac, generate_auth_nonce,
    serialize_message, LanSyncMessage, ProtocolOffer, PROTOCOL_VERSION,
};

pub(super) struct ClientHello {
    pub(super) message: String,
    pub(super) offer: ProtocolOffer,
    pub(super) auth_nonce: Option<String>,
}

pub(super) fn build_client_hello(
    device_id: &str,
    device_name: &str,
    space_id: &str,
    addresses: &[String],
    auth_secret: Option<&str>,
) -> ClientHello {
    let (auth_nonce, auth_hmac) = match auth_secret {
        Some(secret) => {
            let nonce = generate_auth_nonce();
            let hmac = compute_hello_auth_hmac(secret, space_id, device_id, &nonce);
            (Some(nonce), Some(hmac))
        }
        None => (None, None),
    };
    let offer = ProtocolOffer::current_with_registry_history(true);
    let negotiation_hmac = auth_secret
        .zip(auth_nonce.as_deref())
        .map(|(secret, nonce)| {
            compute_negotiation_offer_hmac(secret, space_id, device_id, nonce, &offer)
        });
    let hello = LanSyncMessage::Hello {
        protocol_version: PROTOCOL_VERSION,
        device_id: device_id.to_owned(),
        device_name: device_name.to_owned(),
        space_id: space_id.to_owned(),
        addresses: Some(addresses.to_owned()),
        auth_nonce: auth_nonce.clone(),
        auth_hmac,
        platform: Some(crate::host::local_platform()),
        app_version: crate::host::app_version(),
        supported_protocol_versions: Some(offer.version_ids()),
        supported_capabilities: Some(offer.capability_ids()),
        negotiated_capabilities: None,
        negotiation_hmac,
    };
    ClientHello {
        message: serialize_message(&hello),
        offer,
        auth_nonce,
    }
}
