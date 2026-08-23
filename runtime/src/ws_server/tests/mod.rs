use super::*;
use crate::app_index::{App, AppKind};
use tokio::io::AsyncReadExt;

mod handlers;
mod handshake;
mod lifecycle;
mod lifecycle_requests;
mod lifecycle_shutdown;
mod support;
use support::*;

pub(super) fn baseline_hello() -> HelloMessage {
    HelloMessage {
        kind: Some("hello".into()),
        protocol_version: None,
        api_version: Some(API_VERSION.into()),
        token: Some("test-token".into()),
        pid: Some(std::process::id()),
        client_id: Some("eden".into()),
        client_class: Some("@kosmos/ark".into()),
        client_version: Some("0.1.0".into()),
    }
}

pub(super) fn accepted_compat(outcome: HelloOutcome) -> Compatibility {
    let HelloOutcome::Accept { compatibility, .. } = outcome else {
        assert!(
            matches!(outcome, HelloOutcome::Accept { .. }),
            "expected accept"
        );
        unreachable!();
    };
    compatibility
}

pub(super) fn rejected_code(outcome: HelloOutcome) -> &'static str {
    let HelloOutcome::Reject { code, .. } = outcome else {
        assert!(
            matches!(outcome, HelloOutcome::Reject { .. }),
            "expected reject"
        );
        unreachable!();
    };
    code
}
