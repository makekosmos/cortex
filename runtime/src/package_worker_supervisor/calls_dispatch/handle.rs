use super::super::*;
use super::dispatch;

pub(in crate::package_worker_supervisor) async fn handle_call(
    inner: Arc<SupervisorInner>,
    key: (String, String),
    call: CallMessage,
) {
    let (grant, broker, integration, stdin) = {
        let mut workers = lock(&inner.workers);
        let Some(worker) = workers.get_mut(&key) else {
            return;
        };
        if worker.health.state != WorkerState::Running
            || worker.generation != call.generation
            || worker.in_flight >= MAX_IN_FLIGHT
        {
            send_result(worker.stdin.as_ref(), &call.id, false, None, "unavailable");
            return;
        }
        worker.in_flight += 1;
        (
            worker.grant.clone(),
            worker.broker.clone(),
            worker
                .launch_spec
                .as_ref()
                .and_then(|spec| spec.integration.as_ref())
                .map(|config| config.manifest.clone()),
            worker.stdin.clone(),
        )
    };
    let result = match grant {
        Some(grant) => time::timeout(
            PackageWorkerSupervisor::timeout(),
            dispatch(&inner, &grant, &broker, integration.as_ref(), &call),
        )
        .await
        .unwrap_or_else(|_| Err("timeout")),
        None => Err("forbidden"),
    };
    if let Err(error) = result.as_ref() {
        tracing::warn!(
            target: "package_worker",
            package_id = %key.0,
            version = %key.1,
            operation = ?call.operation,
            error,
            "worker call failed"
        );
    }
    if let Some(stdin) = stdin {
        let mut response = match result {
            Ok(value) => result_line(&call.id, true, Some(value), None),
            Err(error) => result_line(&call.id, false, None, Some(error)),
        };
        if response.len() > MAX_LINE_BYTES {
            response = result_line(&call.id, false, None, Some("unavailable"));
        }
        let current = lock(&inner.workers).get(&key).is_some_and(|worker| {
            worker.health.state == WorkerState::Running && worker.generation == call.generation
        });
        if current {
            let _ = stdin.send(response);
        }
    }
    if let Some(worker) = lock(&inner.workers).get_mut(&key) {
        worker.in_flight = worker.in_flight.saturating_sub(1);
    }
}
