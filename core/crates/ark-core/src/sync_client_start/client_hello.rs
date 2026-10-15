use crate::protocol::{
    compute_hello_auth_hmac, generate_auth_nonce, serialize_message, LanSyncMessage,
    PROTOCOL_VERSION,
};

pub(super) fn serialize_client_hello(
    device_id: &str,
    device_name: &str,
    space_id: &str,
    addresses: &[String],
    auth_secret: Option<&str>,
) -> String {
    let (auth_nonce, auth_hmac) = match auth_secret {
        Some(secret) => {
            let nonce = generate_auth_nonce();
            let hmac = compute_hello_auth_hmac(secret, space_id, device_id, &nonce);
            (Some(nonce), Some(hmac))
        }
        None => (None, None),
    };
    let hello = LanSyncMessage::Hello {
        protocol_version: PROTOCOL_VERSION,
        device_id: device_id.to_owned(),
        device_name: device_name.to_owned(),
        space_id: space_id.to_owned(),
        addresses: Some(addresses.to_owned()),
        auth_nonce,
        auth_hmac,
        platform: Some(crate::host::local_platform()),
        app_version: crate::host::app_version(),
    };
    serialize_message(&hello)
}
