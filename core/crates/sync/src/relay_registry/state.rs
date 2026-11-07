#[derive(Clone, Debug)]
pub(super) enum Admission {
    Waiting,
    Exchanging {
        session_id: String,
        retry_count: u8,
        incoming_page_count: Option<u32>,
        next_page: u32,
        local_digest: Option<String>,
        remote_digest: Option<String>,
    },
    Ready {
        session_id: String,
    },
    Revoked,
}

#[derive(Clone, Debug)]
pub(super) struct RelayRegistryPeer {
    pub(super) endpoint: String,
    pub(super) policy_negotiated: bool,
    pub(super) admission: Admission,
}

impl RelayRegistryPeer {
    pub(super) fn is_ready_on(&self, endpoint: &str, session: Option<&str>) -> bool {
        if self.endpoint != endpoint {
            return false;
        }
        match (&self.admission, session) {
            (Admission::Ready { .. }, None) => true,
            (Admission::Ready { session_id }, Some(expected)) => session_id == expected,
            _ => false,
        }
    }

    pub(super) fn session_id(&self) -> Option<&str> {
        match &self.admission {
            Admission::Exchanging { session_id, .. } | Admission::Ready { session_id } => {
                Some(session_id)
            }
            Admission::Waiting | Admission::Revoked => None,
        }
    }
}

pub(super) fn exchanging(session_id: String, retry_count: u8) -> Admission {
    Admission::Exchanging {
        session_id,
        retry_count,
        incoming_page_count: None,
        next_page: 0,
        local_digest: None,
        remote_digest: None,
    }
}

pub(super) fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
