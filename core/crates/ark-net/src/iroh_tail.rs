// ---------------------------------------------------------------------------
// Tests — unit-тесты (ticket codec, registry).
// ---------------------------------------------------------------------------

#[cfg(test)]
mod ticket_tests {
    use std::net::{Ipv4Addr, SocketAddr};

    use iroh::{EndpointAddr, SecretKey};

    use super::{from_ticket, IrohTransport};

    fn sample_addr() -> EndpointAddr {
        let secret = SecretKey::generate();
        let socket: SocketAddr = (Ipv4Addr::LOCALHOST, 4242).into();
        EndpointAddr::new(secret.public()).with_ip_addr(socket)
    }

    #[test]
    fn ticket_round_trips_through_string() {
        let addr = sample_addr();
        let expected_id = addr.id;

        let ticket_str = IrohTransport::ticket_string_for_addr(&addr);
        let parsed = from_ticket(&ticket_str).expect("ticket should parse back");

        assert_eq!(
            parsed.id, expected_id,
            "round-tripped ticket should preserve EndpointId"
        );
    }

    #[test]
    fn from_ticket_rejects_garbage_string() {
        let result = from_ticket("not-a-real-ticket");
        assert!(result.is_err(), "garbage string must not parse as a ticket");
    }
}

#[cfg(test)]
mod registry_tests {
    use iroh::SecretKey;

    use super::DeviceRegistry;

    #[test]
    fn resolves_device_id_by_endpoint_id_after_insert() {
        let registry = DeviceRegistry::new();
        let endpoint_id = SecretKey::generate().public();

        registry.insert(endpoint_id, "device-A".to_string());

        assert_eq!(
            registry.device_id_for(&endpoint_id),
            Some("device-A".to_string())
        );
    }

    #[test]
    fn resolves_endpoint_id_by_device_id_after_insert() {
        let registry = DeviceRegistry::new();
        let endpoint_id = SecretKey::generate().public();

        registry.insert(endpoint_id, "device-A".to_string());

        assert_eq!(registry.endpoint_id_for("device-A"), Some(endpoint_id));
    }

    #[test]
    fn replacing_endpoint_clears_old_trust() {
        let registry = DeviceRegistry::new();
        let old = SecretKey::generate().public();
        let new = SecretKey::generate().public();
        registry.insert(old, "device-A".to_string());
        assert!(registry.bind_peer_for_control("device-A", &old.to_string()));
        registry.insert(new, "device-A".to_string());
        assert!(registry.data_ready_endpoint("device-A").is_none());
    }

    #[test]
    fn unknown_endpoint_id_resolves_to_none() {
        let registry = DeviceRegistry::new();
        let endpoint_id = SecretKey::generate().public();

        assert_eq!(registry.device_id_for(&endpoint_id), None);
    }

    #[test]
    fn later_insert_overwrites_earlier_mapping_for_same_endpoint_id() {
        let registry = DeviceRegistry::new();
        let endpoint_id = SecretKey::generate().public();

        registry.insert(endpoint_id, "device-A".to_string());
        registry.insert(endpoint_id, "device-A-renamed".to_string());

        assert_eq!(
            registry.device_id_for(&endpoint_id),
            Some("device-A-renamed".to_string())
        );
        assert_eq!(registry.endpoint_id_for("device-A"), None);
    }

    #[test]
    fn untrusted_hello_cannot_replace_trusted_endpoint_mapping() {
        let registry = DeviceRegistry::new();
        let endpoint_id = SecretKey::generate().public();

        registry.insert(endpoint_id, "device-A".to_string());
        assert!(registry.bind_peer_for_control("device-A", &endpoint_id.to_string()));
        assert!(registry.mark_data_ready("device-A", &endpoint_id.to_string()));

        assert!(!registry.insert_untrusted(endpoint_id, "device-attacker".to_string()));
        assert_eq!(
            registry.device_id_for(&endpoint_id).as_deref(),
            Some("device-A")
        );
        assert_eq!(registry.data_ready_endpoint("device-A"), Some(endpoint_id));
    }

    #[test]
    fn untrusted_hello_cannot_move_trusted_device_to_new_endpoint() {
        let registry = DeviceRegistry::new();
        let old_endpoint = SecretKey::generate().public();
        let new_endpoint = SecretKey::generate().public();

        registry.insert(old_endpoint, "device-A".to_string());
        assert!(registry.bind_peer_for_control("device-A", &old_endpoint.to_string()));
        assert!(registry.mark_data_ready("device-A", &old_endpoint.to_string()));

        assert!(!registry.insert_untrusted(new_endpoint, "device-A".to_string()));
        assert_eq!(registry.endpoint_id_for("device-A"), Some(old_endpoint));
        assert_eq!(registry.data_ready_endpoint("device-A"), Some(old_endpoint));
    }

    #[test]
    fn endpoint_binding_stays_closed_to_data_until_explicit_admission() {
        let registry = DeviceRegistry::new();
        let endpoint_id = SecretKey::generate().public();
        registry.insert(endpoint_id, "device-A".to_string());

        assert!(registry.bind_peer_for_control("device-A", &endpoint_id.to_string()));
        assert!(!registry.is_data_ready_endpoint(&endpoint_id));
        assert!(!registry.insert_untrusted(endpoint_id, "attacker".to_string()));

        assert!(registry.mark_data_ready("device-A", &endpoint_id.to_string()));
        assert!(registry.is_data_ready_endpoint(&endpoint_id));

        registry.unbind_peer("device-A");
        assert!(!registry.is_data_ready_endpoint(&endpoint_id));
    }

    #[test]
    fn repeated_hello_keeps_the_existing_peer_phase() {
        let registry = DeviceRegistry::new();
        let endpoint_id = SecretKey::generate().public();
        registry.insert(endpoint_id, "device-A".to_string());
        assert!(registry.bind_peer_for_control("device-A", &endpoint_id.to_string()));

        assert!(registry.insert_untrusted(endpoint_id, "device-A".to_string()));
        assert!(registry.mark_data_ready("device-A", &endpoint_id.to_string()));
        assert!(registry.insert_untrusted(endpoint_id, "device-A".to_string()));

        assert!(registry.is_data_ready_endpoint(&endpoint_id));
    }

    #[test]
    fn inserting_existing_device_removes_its_old_endpoint_mapping() {
        let registry = DeviceRegistry::new();
        let old_endpoint = SecretKey::generate().public();
        let new_endpoint = SecretKey::generate().public();

        registry.insert(old_endpoint, "device-A".to_string());
        registry.insert(new_endpoint, "device-A".to_string());

        assert_eq!(registry.device_id_for(&old_endpoint), None);
        assert_eq!(registry.endpoint_id_for("device-A"), Some(new_endpoint));
    }
}

#[cfg(test)]
mod relay_connectivity_tests {
    use super::relay_connectivity_change;

    /// The watcher fires on every retry (each `Disconnected` carries a fresh
    /// error) — only real up/down transitions must produce a new state, so
    /// the log keeps exactly one warn per flap (KOS-298).
    #[test]
    fn relay_connectivity_logs_only_on_state_transitions() {
        // First observation records state without reporting a flap.
        assert_eq!(relay_connectivity_change(None, &[false]), Some(false));
        // Retrying while still down yields no transition.
        assert_eq!(relay_connectivity_change(Some(false), &[false]), None);
        // up after down — one transition.
        assert_eq!(relay_connectivity_change(Some(false), &[true]), Some(true));
        // Still up — nothing.
        assert_eq!(relay_connectivity_change(Some(true), &[true]), None);
        // Any relay connected counts as connected.
        assert_eq!(relay_connectivity_change(Some(true), &[false, true]), None);
        // All relays lost — down transition.
        assert_eq!(
            relay_connectivity_change(Some(true), &[false, false]),
            Some(false)
        );
        // No home relay selected yet keeps the previous state.
        assert_eq!(relay_connectivity_change(Some(true), &[]), Some(true));
        assert_eq!(relay_connectivity_change(None, &[]), None);
    }
}
