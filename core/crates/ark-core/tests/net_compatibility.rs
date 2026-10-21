use ark_core::net::{
    filter_routable_addresses, is_address_routable, is_loopback_address, is_routable_v4,
    is_routable_v4_addr, is_routable_v6, is_routable_v6_addr, is_virtual_interface,
    VIRTUAL_IFACE_PREFIXES,
};

#[test]
fn net_policy_remains_available_from_ark_core() {
    assert!(is_routable_v4("192.168.1.5"));
    assert!(is_routable_v4_addr(std::net::Ipv4Addr::new(192, 168, 1, 5)));
    assert!(is_routable_v6("2001:db8::1"));
    assert!(is_routable_v6_addr(
        "2001:db8::1".parse().expect("valid IPv6 literal")
    ));
    assert!(is_address_routable("[2001:db8::1]:21531"));
    assert!(is_loopback_address("127.0.0.1:21531"));
    assert!(is_virtual_interface("utun0"));
    assert!(VIRTUAL_IFACE_PREFIXES.contains(&"utun"));
    assert_eq!(
        filter_routable_addresses(&["192.168.1.5:21531".into()]),
        ["192.168.1.5:21531"]
    );
}
