use async_trait::async_trait;
use engine::{
    package_worker_supervisor::{ArkRequestExecutor, PackageWorkerSupervisor},
    runtime_grants::{
        CompileInput, DataRequest, GrantCompiler, GrantRule, LaunchGrant, RegisteredType,
        RegistrySnapshot,
    },
};
use serde_json::json;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct RecordingArk {
    calls: Mutex<Vec<(String, serde_json::Value)>>,
}

#[async_trait]
impl ArkRequestExecutor for RecordingArk {
    async fn request(
        &self,
        operation: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, &'static str> {
        self.calls
            .lock()
            .expect("test prerequisite")
            .push((operation.to_owned(), params));
        Ok(json!({"ok": true}))
    }
}

fn grant() -> LaunchGrant {
    GrantCompiler::compile(
        CompileInput::new(
            "pkg",
            "1.0.0",
            "manifest-digest",
            vec![GrantRule {
                type_id: "note".into(),
                versions: vec!["1.0.0".into()],
                actions: ["read".into()].into_iter().collect(),
                fields_read: vec!["title".into()],
                fields_write: vec![],
                relations_read: vec![],
                relations_write: vec![],
            }],
        ),
        &RegistrySnapshot::new(vec![RegisteredType::new("note", "1.0.0", &["title"], &[])]),
    )
    .expect("test prerequisite")
}

fn read_request(field: &str) -> serde_json::Value {
    serde_json::to_value(DataRequest::ReadObject {
        type_id: "note".into(),
        type_version: "1.0.0".into(),
        object_id: "n1".into(),
        fields: vec![field.into()],
        relations: vec![],
    })
    .expect("test prerequisite")
}

#[tokio::test]
async fn supervisor_authority_denies_without_forwarding_and_allows_bound_generation() {
    let ark = Arc::new(RecordingArk::default());
    let supervisor = PackageWorkerSupervisor::with_ark_executor(1, ark.clone());
    supervisor
        .bind_typed_launch("pkg", "1.0.0", "session-1", 7, grant())
        .expect("test prerequisite");

    let denied = supervisor
        .dispatch_typed_request("pkg", "1.0.0", "session-1", 7, read_request("secret"))
        .await;
    assert_eq!(denied, Err("forbidden"));
    assert!(ark.calls.lock().expect("test prerequisite").is_empty());

    let stale = supervisor
        .dispatch_typed_request("pkg", "1.0.0", "session-1", 6, read_request("title"))
        .await;
    assert_eq!(stale, Err("stale-generation"));
    assert!(ark.calls.lock().expect("test prerequisite").is_empty());

    let malformed = supervisor
        .dispatch_typed_request(
            "pkg",
            "1.0.0",
            "session-1",
            7,
            json!({"kind":"read_object","type_id":"note","type_version":"1.0.0","object_id":"n1","fields":[],"relations":[],"unknown":true}),
        )
        .await;
    assert_eq!(malformed, Err("invalid-request"));
    assert!(ark.calls.lock().expect("test prerequisite").is_empty());

    let allowed = supervisor
        .dispatch_typed_request("pkg", "1.0.0", "session-1", 7, read_request("title"))
        .await;
    assert_eq!(allowed, Ok(json!({"ok": true})));
    assert_eq!(ark.calls.lock().expect("test prerequisite").len(), 1);

    supervisor.revoke_typed_launch("pkg", "1.0.0");
    assert_eq!(
        supervisor
            .dispatch_typed_request("pkg", "1.0.0", "session-1", 7, read_request("title"))
            .await,
        Err("forbidden")
    );
    assert_eq!(ark.calls.lock().expect("test prerequisite").len(), 1);
    // Drain every supervisor registry so no task keeps a worker child or
    // an ArkHost handle alive past the test's tempdir cleanup (KOS-270).
    let _ = supervisor.stop_all().await;
}
