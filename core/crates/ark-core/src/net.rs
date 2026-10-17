//! Compatibility facade for the shared address policy.

pub use ark_net_policy::{
    filter_routable_addresses, is_address_routable, is_loopback_address, is_routable_v4,
    is_routable_v4_addr, is_routable_v6, is_routable_v6_addr, is_virtual_interface,
    VIRTUAL_IFACE_PREFIXES,
};
