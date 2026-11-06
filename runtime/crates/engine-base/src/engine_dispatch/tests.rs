use super::*;
use serde_json::json;
use std::collections::HashSet;
use std::sync::Mutex;

#[test]
fn owner_allocator_never_returns_zero_and_skips_wrap() {
    let allocator = OwnerAllocator::default();
    assert_ne!(allocator.allocate().unwrap().id(), 0);
    let wrapping = OwnerAllocator::with_next(u64::MAX);
    assert_eq!(wrapping.allocate().unwrap().id(), u64::MAX);
    assert_eq!(wrapping.allocate().unwrap().id(), 1);
    assert_ne!(wrapping.allocate().unwrap().id(), 0);
}

#[test]
fn owner_allocator_skips_live_owner_across_wrap_and_reuses_only_after_release() {
    let allocator = OwnerAllocator::with_next_and_live(u64::MAX, &[1]);
    assert_eq!(allocator.allocate().unwrap().id(), u64::MAX);
    let wrapped = allocator.allocate().unwrap();
    assert_eq!(wrapped.id(), 2);
    allocator.release(1);
    allocator.set_next_for_test(1);
    assert_eq!(allocator.allocate().unwrap().id(), 1);
}

#[test]
fn owner_allocator_is_unique_under_concurrency() {
    let allocator = OwnerAllocator::default();
    let owners = Arc::new(Mutex::new(Vec::new()));
    std::thread::scope(|scope| {
        for _ in 0..16 {
            let allocator = allocator.clone();
            let owners = owners.clone();
            scope.spawn(move || {
                let mut local = Vec::with_capacity(1_000);
                for _ in 0..1_000 {
                    local.push(allocator.allocate().unwrap().id());
                }
                owners.lock().unwrap().extend(local);
            });
        }
    });
    let owners = owners.lock().unwrap();
    assert_eq!(owners.len(), 16_000);
    assert_eq!(
        owners.iter().copied().collect::<HashSet<_>>().len(),
        owners.len()
    );
    assert!(!owners.contains(&0));
}

#[test]
fn owner_lease_releases_live_owner_idempotently_on_drop() {
    let allocator = OwnerAllocator::default();
    let lease = allocator.allocate().expect("owner lease");
    assert_eq!(allocator.live_count(), 1);
    assert_eq!(lease.id(), 1);
    lease.release();
    lease.release();
    assert_eq!(allocator.live_count(), 0);
}

#[test]
fn owner_lease_can_be_transferred_without_releasing_until_guard_drop() {
    let allocator = OwnerAllocator::default();
    let lease = allocator.allocate().expect("owner lease");
    let owner = lease;
    assert_eq!(allocator.live_count(), 1);
    drop(owner);
    assert_eq!(allocator.live_count(), 0);
}

#[test]
fn cleanup_connection_releases_lease_before_drop() {
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let seen = calls.clone();
    let dispatcher = EngineDispatcher::with_cleanup(
        Arc::new(|_| Box::pin(async { Ok(json!({"ok": true})) })),
        Arc::new(move |_| {
            seen.fetch_add(1, Ordering::SeqCst);
        }),
    );
    let lease = dispatcher.allocate_owner().unwrap();
    dispatcher.cleanup_connection_sync(&lease);
    dispatcher.cleanup_connection_sync(&lease);
    assert_eq!(dispatcher.live_owner_count(), 0);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn validates_wire_request_into_typed_dispatch_request() {
    let request = DispatchRequest::from_wire(json!({
        "_req_id": "req-1",
        "operation": "ark.read",
        "params": {"id": "object-1"}
    }))
    .expect("valid request");

    assert_eq!(request.request_id.as_deref(), Some("req-1"));
    assert_eq!(request.operation, Operation::Named("ark.read".into()));
    assert_eq!(request.params, json!({"params": {"id": "object-1"}}));
}

#[test]
fn rejects_malformed_and_unknown_shape_at_the_boundary() {
    assert_eq!(
        DispatchRequest::from_wire(json!({"operation": " "})),
        Err(DispatchValidationError::InvalidOperation)
    );
    assert_eq!(
        DispatchRequest::from_wire(json!({"operation": 42})),
        Err(DispatchValidationError::InvalidOperation)
    );
    assert_eq!(
        DispatchRequest::from_wire(json!({"operation": "ark.read", "_req_id": 1})),
        Err(DispatchValidationError::InvalidRequestId)
    );
}

#[test]
fn preserves_explicit_params_and_mixed_siblings_as_legacy_payload() {
    let request = DispatchRequest::from_wire(json!({
        "_req_id": "req-1",
        "operation": "commands.invoke",
        "id": "command-id",
        "params": {"nested": true},
        "extra": 42
    }))
    .expect("valid request");

    assert_eq!(request.request_id.as_deref(), Some("req-1"));
    assert_eq!(
        request.params,
        json!({
            "id": "command-id",
            "params": {"nested": true},
            "extra": 42
        })
    );
}

fn request(operation: &str, params: Value) -> DispatchRequest {
    DispatchRequest::from_wire(json!({
        "id": "test-1",
        "operation": operation,
        "params": params,
    }))
    .unwrap()
}

#[tokio::test]
async fn dispatcher_covers_read_write_unknown_structured_and_event_paths() {
    let events = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let events_for_handler = events.clone();
    let dispatcher = EngineDispatcher::new(Arc::new(move |request| {
        let events = events_for_handler.clone();
        Box::pin(async move {
            match request.operation.as_str() {
                "read" => Ok(json!({"ok": true, "value": 7})),
                "write" => Ok(json!({"ok": true, "written": true})),
                "event" => {
                    events.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    Ok(json!({"ok": true, "event": "changed"}))
                }
                "structured-error" => Err(DispatchError::Failed("bad params".into())),
                _ => Err(DispatchError::Failed("unknown operation".into())),
            }
        })
    }));

    assert_eq!(
        dispatcher
            .dispatch(request("read", json!({})))
            .await
            .unwrap()["value"],
        7
    );
    assert_eq!(
        dispatcher
            .dispatch(request("write", json!({"x": 1})))
            .await
            .unwrap()["written"],
        true
    );
    assert_eq!(
        dispatcher
            .dispatch(request("event", json!({})))
            .await
            .unwrap()["event"],
        "changed"
    );
    assert_eq!(events.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(
        dispatcher.dispatch(request("unknown", json!({}))).await,
        Err(DispatchError::Failed("unknown operation".into()))
    );
    assert_eq!(
        dispatcher
            .dispatch(request("structured-error", json!(null)))
            .await,
        Err(DispatchError::Failed("bad params".into()))
    );
}

#[tokio::test]
async fn dispatcher_timeout_and_cancellation_are_bounded_by_caller() {
    let dispatcher = EngineDispatcher::new(Arc::new(|_| {
        Box::pin(async {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            Ok(json!({"ok": true}))
        })
    }));
    let timed = tokio::time::timeout(
        std::time::Duration::from_millis(10),
        dispatcher.dispatch(request("slow", json!({}))),
    )
    .await;
    assert!(timed.is_err());

    let dispatcher_for_task = dispatcher.clone();
    let task = tokio::spawn(async move {
        dispatcher_for_task
            .dispatch(request("slow", json!({})))
            .await
    });
    task.abort();
    assert!(task
        .await
        .expect_err("task must be cancelled")
        .is_cancelled());
}
