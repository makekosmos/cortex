use std::sync::Arc;

use serde_json::Value;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

pub(super) const REQUEST_QUEUE_CAPACITY: usize = 64;
const REQUEST_QUEUE_BYTES: usize = 64 * 1024 * 1024;
const MAX_REQUEST_BYTES: usize = 16 * 1024 * 1024;
const MAX_VALUE_DEPTH: usize = 64;
const MAX_VALUE_NODES: usize = 100_000;
const VALUE_NODE_OVERHEAD: usize = 128;
const VALUE_STRING_OVERHEAD: usize = 64;

#[derive(Debug, thiserror::Error)]
pub(super) enum AdmissionError {
    #[error("ark request exceeds admission size limits")]
    Oversized,
    #[error("ark request queue is full")]
    Saturated,
}

pub(super) struct RequestAdmission {
    count: Arc<Semaphore>,
    bytes: Arc<Semaphore>,
}

pub(super) struct AdmissionPermit {
    _count: OwnedSemaphorePermit,
    _bytes: OwnedSemaphorePermit,
}

impl RequestAdmission {
    pub(super) fn new() -> Self {
        Self {
            count: Arc::new(Semaphore::new(REQUEST_QUEUE_CAPACITY)),
            bytes: Arc::new(Semaphore::new(REQUEST_QUEUE_BYTES)),
        }
    }

    pub(super) fn try_admit(
        &self,
        operation: &str,
        params: &Value,
    ) -> Result<AdmissionPermit, AdmissionError> {
        let mut nodes = 0;
        let params_cost = estimate_value(params, 0, &mut nodes)?;
        let cost = params_cost
            .checked_add(operation.len().saturating_mul(2))
            .and_then(|value| value.checked_add(128))
            .ok_or(AdmissionError::Oversized)?;
        if cost > MAX_REQUEST_BYTES {
            return Err(AdmissionError::Oversized);
        }
        self.try_admit_cost(cost)
    }

    fn try_admit_cost(&self, cost: usize) -> Result<AdmissionPermit, AdmissionError> {
        if cost == 0 || cost > MAX_REQUEST_BYTES {
            return Err(AdmissionError::Oversized);
        }
        let count = self
            .count
            .clone()
            .try_acquire_owned()
            .map_err(|_| AdmissionError::Saturated)?;
        let units = u32::try_from(cost).map_err(|_| AdmissionError::Oversized)?;
        let bytes = match self.bytes.clone().try_acquire_many_owned(units) {
            Ok(permit) => permit,
            Err(_) => return Err(AdmissionError::Saturated),
        };
        Ok(AdmissionPermit {
            _count: count,
            _bytes: bytes,
        })
    }
}

fn estimate_value(value: &Value, depth: usize, nodes: &mut usize) -> Result<usize, AdmissionError> {
    *nodes = nodes.checked_add(1).ok_or(AdmissionError::Oversized)?;
    if depth > MAX_VALUE_DEPTH || *nodes > MAX_VALUE_NODES {
        return Err(AdmissionError::Oversized);
    }

    let base = match value {
        Value::Null => VALUE_NODE_OVERHEAD,
        Value::Bool(_) => VALUE_NODE_OVERHEAD,
        Value::Number(_) => VALUE_NODE_OVERHEAD,
        Value::String(value) => value
            .len()
            .checked_mul(4)
            .and_then(|size| size.checked_add(VALUE_STRING_OVERHEAD))
            .ok_or(AdmissionError::Oversized)?,
        Value::Array(values) => {
            let mut total = VALUE_NODE_OVERHEAD;
            for value in values {
                total = total
                    .checked_add(estimate_value(value, depth + 1, nodes)?)
                    .ok_or(AdmissionError::Oversized)?;
                if total > MAX_REQUEST_BYTES {
                    return Err(AdmissionError::Oversized);
                }
            }
            total
        }
        Value::Object(values) => {
            let mut total = VALUE_NODE_OVERHEAD;
            for (key, value) in values {
                let key_cost = key
                    .len()
                    .checked_mul(4)
                    .and_then(|size| size.checked_add(VALUE_STRING_OVERHEAD))
                    .ok_or(AdmissionError::Oversized)?;
                let value_cost = estimate_value(value, depth + 1, nodes)?;
                total = total
                    .checked_add(key_cost)
                    .and_then(|size| size.checked_add(value_cost))
                    .ok_or(AdmissionError::Oversized)?;
                if total > MAX_REQUEST_BYTES {
                    return Err(AdmissionError::Oversized);
                }
            }
            total
        }
    };
    Ok(base)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn admission_bounds_count_and_releases_capacity() {
        let admission = RequestAdmission {
            count: Arc::new(Semaphore::new(2)),
            bytes: Arc::new(Semaphore::new(4096)),
        };
        let first = admission.try_admit_cost(100).expect("first permit");
        let second = admission.try_admit_cost(100).expect("second permit");
        assert!(matches!(
            admission.try_admit_cost(100),
            Err(AdmissionError::Saturated)
        ));
        drop(first);
        let _third = admission
            .try_admit_cost(100)
            .expect("released count permit");
        drop(second);
    }

    #[test]
    fn admission_bounds_bytes_and_releases_capacity() {
        let admission = RequestAdmission {
            count: Arc::new(Semaphore::new(4)),
            bytes: Arc::new(Semaphore::new(200)),
        };
        let first = admission.try_admit_cost(150).expect("first permit");
        assert!(matches!(
            admission.try_admit_cost(100),
            Err(AdmissionError::Saturated)
        ));
        drop(first);
        let _second = admission.try_admit_cost(100).expect("released byte permit");
    }

    #[test]
    fn oversized_depth_and_node_count_are_rejected() {
        let admission = RequestAdmission::new();
        assert!(matches!(
            admission.try_admit_cost(MAX_REQUEST_BYTES + 1),
            Err(AdmissionError::Oversized)
        ));

        let mut nested = json!(null);
        for _ in 0..=MAX_VALUE_DEPTH {
            nested = Value::Array(vec![nested]);
        }
        assert!(matches!(
            admission.try_admit("write", &nested),
            Err(AdmissionError::Oversized)
        ));

        let many_nodes = Value::Array(vec![Value::Null; MAX_VALUE_NODES]);
        assert!(matches!(
            admission.try_admit("write", &many_nodes),
            Err(AdmissionError::Oversized)
        ));
    }
}
