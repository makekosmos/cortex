mod lifecycle;
mod registry_exchange;
mod registry_fanout;
mod task_merge;
mod transport_events;

#[cfg(test)]
#[path = "relay_sync/pairing_retry_tests.rs"]
mod pairing_retry_tests;

#[cfg(test)]
#[path = "relay_sync/revoked_device_catchup_tests.rs"]
mod revoked_device_catchup_tests;

include!("relay_types.rs");
include!("relay_impl_1.rs");
include!("relay_impl_2.rs");
include!("relay_disconnect.rs");
include!("relay_pairing.rs");
include!("relay_helpers.rs");
