use std::collections::HashSet;

use super::Request;

#[derive(serde::Deserialize)]
struct RequestFixture {
    name: String,
    request: String,
}

#[test]
fn ark_api_v1_requests_remain_deserializable() {
    let fixtures = include_str!("../../../tests/fixtures/ark-api-v1/requests.jsonl");
    let mut seen = HashSet::new();
    for line in fixtures.lines() {
        let fixture: RequestFixture = serde_json::from_str(line).unwrap();
        let request: Request = serde_json::from_str(&fixture.request)
            .unwrap_or_else(|error| panic!("{} did not decode: {error}", fixture.name));
        match fixture.name.as_str() {
            "canonical-object-upsert" => match request {
                Request::UpsertObject {
                    object,
                    expected_snapshot,
                    ..
                } => {
                    assert_eq!(object.id, "object-a");
                    let snapshot = expected_snapshot.expect("expectedSnapshot should decode");
                    assert!(snapshot.exists);
                    assert_eq!(snapshot.revision.as_deref(), Some("revision-a"));
                }
                other => panic!("expected UpsertObject, got {other:?}"),
            },
            "canonical-object-delete" => match request {
                Request::DeleteObject {
                    id,
                    expected_snapshot,
                    ..
                } => {
                    assert_eq!(id, "object-a");
                    assert!(expected_snapshot.is_some());
                }
                other => panic!("expected DeleteObject, got {other:?}"),
            },
            "pairing-code-alias" => match request {
                Request::ConnectWithPairingCode { pairing_code } => {
                    assert_eq!(pairing_code, "ABCD-EFGH");
                }
                other => panic!("expected ConnectWithPairingCode, got {other:?}"),
            },
            "integration-send-signed-sync" => match request {
                Request::IntegrationSendSignedSync { frame } => {
                    assert_eq!(frame.space_id, "space-a");
                    assert_eq!(frame.origin_node_id, "node-a");
                    assert_eq!(frame.recipient_node_id, "node-b");
                    assert_eq!(frame.message_id, "message-a");
                    assert_eq!(frame.signature, "00");
                }
                other => panic!("expected IntegrationSendSignedSync, got {other:?}"),
            },
            other => panic!("unknown fixture {other}"),
        }
        assert!(seen.insert(fixture.name), "duplicate request fixture");
    }
    assert_eq!(
        seen,
        [
            "canonical-object-upsert",
            "canonical-object-delete",
            "pairing-code-alias",
            "integration-send-signed-sync",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    );
}

#[test]
fn object_mutation_operation_id_is_optional_and_separate_from_request_correlation() {
    let request_json = concat!(
        r#"{"operation":"delete_object","id":"object-a",""#,
        r#"operationId":"retry-17","_req_id":"request-42"}"#,
    );
    let request: Request = serde_json::from_str(request_json).unwrap();
    assert!(matches!(
        request,
        Request::DeleteObject {
            id,
            operation_id: Some(operation_id),
            ..
        } if id == "object-a" && operation_id == "retry-17"
    ));
}
