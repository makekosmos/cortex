mod lifecycle;
mod registry_exchange;
mod transport_events;

#[cfg(test)]
#[path = "relay_sync/pairing_retry_tests.rs"]
mod pairing_retry_tests;

include!("relay_types.rs");
include!("relay_impl_1.rs");
include!("relay_impl_2.rs");
include!("relay_pairing.rs");
include!("relay_helpers.rs");
