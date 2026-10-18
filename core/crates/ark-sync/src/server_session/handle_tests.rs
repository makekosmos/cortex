use std::num::NonZeroUsize;

use super::{
    ServerPeerProjection, ServerPeerReadiness, ServerSessionActivationError, ServerSessionControl,
    ServerSessionHandle, ServerSessionIntent, ServerSessionIntentError,
};

#[tokio::test]
async fn lease_matches_only_its_exact_session() {
    let capacity = NonZeroUsize::new(1).expect("one is nonzero");
    let (first, _first_actor, first_lease, _) =
        ServerSessionHandle::channel(capacity).expect("session identity is available");
    let (_second, second_actor, second_lease, _) =
        ServerSessionHandle::channel(capacity).expect("session identity is available");

    assert!(first.matches_lease(first_lease));
    assert!(first_lease.matches(&first.lease()));
    assert!(!first.matches_lease(second_lease));
    assert!(!second_actor.lease().matches(&first_lease));
}

#[tokio::test]
async fn activation_is_one_shot_and_immediate_revoke_strengthens_close_after_flush() {
    let (handle, actor, lease, mut activation) =
        ServerSessionHandle::channel(NonZeroUsize::new(1).expect("one is nonzero"))
            .expect("session identity is available");
    assert_eq!(activation.lease(), lease);

    activation.activate().expect("pending session activates");
    assert_eq!(actor.control(), ServerSessionControl::Active);
    assert_eq!(
        activation.activate(),
        Err(ServerSessionActivationError::AlreadyUsed)
    );
    assert!(handle.close_after_flush());
    assert!(handle.revoke());
    assert!(!handle.revoke());
    assert_eq!(actor.control(), ServerSessionControl::CloseNow);
}

#[tokio::test]
async fn close_after_flush_prevents_later_activation() {
    let (handle, actor, _, mut activation) =
        ServerSessionHandle::channel(NonZeroUsize::new(1).expect("one is nonzero"))
            .expect("session identity is available");

    assert!(handle.close_after_flush());
    assert_eq!(
        activation.activate(),
        Err(ServerSessionActivationError::Revoked)
    );
    assert_eq!(actor.control(), ServerSessionControl::CloseAfterFlush);
}

#[tokio::test]
async fn immediate_revoke_prevents_later_activation() {
    let (handle, actor, _, mut activation) =
        ServerSessionHandle::channel(NonZeroUsize::new(1).expect("one is nonzero"))
            .expect("session identity is available");

    assert!(handle.revoke());
    assert_eq!(
        activation.activate(),
        Err(ServerSessionActivationError::Revoked)
    );
    assert_eq!(actor.control(), ServerSessionControl::CloseNow);
}

#[tokio::test]
async fn projection_updates_are_visible_to_core() {
    let (handle, actor, _, _) =
        ServerSessionHandle::channel(NonZeroUsize::new(1).expect("one is nonzero"))
            .expect("session identity is available");
    let mut updates = handle.projection_updates();
    let projection = ServerPeerProjection {
        device_id: Some("peer-7".to_owned()),
        device_name: Some("Laptop".to_owned()),
        addresses: vec!["127.0.0.1:7".to_owned()],
        platform: Some("linux".to_owned()),
        app_version: Some("1.2.3".to_owned()),
        readiness: ServerPeerReadiness::Ready,
    };

    actor.publish_projection(projection.clone());

    updates.changed().await.expect("projection changed");
    assert_eq!(updates.borrow_and_update().clone(), projection);
    assert_eq!(handle.projection(), projection);
}

#[tokio::test]
async fn close_after_flush_remains_visible_when_intent_mailbox_is_full() {
    let (handle, mut actor, _, _) =
        ServerSessionHandle::channel(NonZeroUsize::new(1).expect("one is nonzero"))
            .expect("session identity is available");
    handle
        .try_send(ServerSessionIntent::WakeCommittedFeed)
        .expect("first intent fits");
    assert_eq!(
        handle.try_send(ServerSessionIntent::WakeTaskMerge),
        Err(ServerSessionIntentError::Full)
    );

    assert!(handle.close_after_flush());

    assert_eq!(
        actor
            .control_changed()
            .await
            .expect("control signal is open"),
        ServerSessionControl::CloseAfterFlush
    );
    assert!(actor.is_revoked());
    assert_eq!(
        handle.try_send(ServerSessionIntent::WakeTaskMerge),
        Err(ServerSessionIntentError::Closed)
    );
}
