impl LocalSttSidecarClient {
    async fn spawn() -> Result<Self, LocalError> {
        let sidecar_path = local_stt_sidecar_path()?;
        let mut command = TokioCommand::new(&sidecar_path);
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // See backend.rs: creation flags travel through ProcessTree::spawn so
        // the suspended child joins the KILL_ON_JOB_CLOSE job before running.
        #[cfg(windows)]
        let creation_flags = CREATE_NO_WINDOW;
        #[cfg(not(windows))]
        let creation_flags = 0;
        let mut tree = ProcessTree::spawn(&mut command, creation_flags)
            .await
            .map_err(|e| {
                LocalError::SidecarUnavailable(format!(
                    "не удалось запустить {}: {e}",
                    sidecar_path.display()
                ))
            })?;

        let child = tree.child_mut();
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| LocalError::SidecarUnavailable("sidecar stdin is unavailable".into()))?;
        let stdout = child.stdout.take().ok_or_else(|| {
            LocalError::SidecarUnavailable("sidecar stdout is unavailable".into())
        })?;

        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    eprintln!("[mundus-local-stt] {line}");
                }
            });
        }

        Ok(Self {
            child: tree,
            stdin,
            stdout: BufReader::new(stdout),
            next_request_id: 1,
        })
    }

    async fn request(&mut self, request: LocalSttRequest) -> Result<LocalSttResponse, LocalError> {
        if let Some(status) = self
            .child
            .child_mut()
            .try_wait()
            .map_err(|e| LocalError::SidecarUnavailable(format!("sidecar wait failed: {e}")))?
        {
            return Err(LocalError::SidecarUnavailable(format!(
                "sidecar exited with status {status}"
            )));
        }

        let request_id = self.next_request_id;
        self.next_request_id += 1;
        let envelope = LocalSttRequestEnvelope {
            request_id,
            request,
        };
        let mut payload = serde_json::to_vec(&envelope).map_err(|e| {
            LocalError::SidecarUnavailable(format!("не удалось сериализовать запрос: {e}"))
        })?;
        payload.push(b'\n');

        self.stdin.write_all(&payload).await.map_err(|e| {
            LocalError::SidecarUnavailable(format!("не удалось отправить sidecar-запрос: {e}"))
        })?;
        self.stdin.flush().await.map_err(|e| {
            LocalError::SidecarUnavailable(format!("не удалось завершить sidecar-запрос: {e}"))
        })?;

        let mut line = String::new();
        let read = self.stdout.read_line(&mut line).await.map_err(|e| {
            LocalError::SidecarUnavailable(format!("не удалось прочитать ответ sidecar: {e}"))
        })?;
        if read == 0 {
            return Err(LocalError::SidecarUnavailable(
                "sidecar stdout closed unexpectedly".into(),
            ));
        }
        let envelope: LocalSttResponseEnvelope =
            serde_json::from_str(line.trim()).map_err(|e| {
                LocalError::SidecarUnavailable(format!("не удалось распарсить ответ sidecar: {e}"))
            })?;
        if envelope.request_id != request_id {
            return Err(LocalError::SidecarUnavailable(format!(
                "unexpected response id {}, expected {request_id}",
                envelope.request_id
            )));
        }
        if !envelope.ok {
            return Err(LocalError::CommandFailed(
                envelope
                    .error
                    .unwrap_or_else(|| "sidecar request failed".into()),
            ));
        }
        envelope.response.ok_or_else(|| {
            LocalError::SidecarUnavailable("sidecar returned empty response payload".into())
        })
    }
}
fn model_spec_from_owned(req: &OwnedLocalRequest) -> LocalSttModelSpec {
    LocalSttModelSpec {
        engine: req.engine.clone(),
        model_id: req.model_id.clone(),
        model_path: req.model_path.clone(),
        command_path: req.command_path.clone(),
        accelerator: req.accelerator.clone(),
        profile: req.profile.clone(),
        // Используем разрешённое значение из запроса (config + env override).
        idle_unload_after_ms: req.idle_unload_ms,
    }
}

fn owned_request_from_model(
    model: &LocalSttModelSpec,
    wav_bytes: &[u8],
    language: &str,
    prompt: &str,
) -> OwnedLocalRequest {
    OwnedLocalRequest {
        wav_bytes: wav_bytes.to_vec(),
        language: language.to_owned(),
        prompt: prompt.to_owned(),
        engine: model.engine.clone(),
        model_id: model.model_id.clone(),
        model_path: model.model_path.clone(),
        command_path: model.command_path.clone(),
        accelerator: model.accelerator.clone(),
        profile: model.profile.clone(),
        idle_unload_ms: model.idle_unload_after_ms,
    }
}

#[cfg(test)]
#[derive(Debug, Clone)]
struct TestSidecarMock {
    transcript: Option<String>,
    fail_error: Option<String>,
    sidecar_unavailable_remaining: u8,
    seen_ops: Vec<String>,
}

#[cfg(test)]
static TEST_SIDECAR_MOCK: OnceLock<Mutex<Option<TestSidecarMock>>> = OnceLock::new();

#[cfg(test)]
pub(crate) static TEST_SIDECAR_TEST_LOCK: tokio::sync::Mutex<()> =
    tokio::sync::Mutex::const_new(());

#[cfg(test)]
fn test_sidecar_mock_state() -> &'static Mutex<Option<TestSidecarMock>> {
    TEST_SIDECAR_MOCK.get_or_init(|| Mutex::new(None))
}

#[cfg(test)]
pub(crate) fn install_test_sidecar_mock_for_host(transcript: Option<String>) {
    let guard = match test_sidecar_mock_state().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let mut guard = guard;
    *guard = Some(TestSidecarMock {
        transcript,
        fail_error: None,
        sidecar_unavailable_remaining: 0,
        seen_ops: Vec::new(),
    });
}

#[cfg(test)]
pub(crate) fn install_test_sidecar_unavailable_for_host(times: u8) {
    let guard = match test_sidecar_mock_state().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let mut guard = guard;
    *guard = Some(TestSidecarMock {
        transcript: None,
        fail_error: None,
        sidecar_unavailable_remaining: times,
        seen_ops: Vec::new(),
    });
}

#[cfg(test)]
pub(crate) fn clear_test_sidecar_mock_for_host() {
    let guard = match test_sidecar_mock_state().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let mut guard = guard;
    *guard = None;
}

#[cfg(test)]
pub(crate) fn has_test_sidecar_mock_for_host() -> bool {
    let guard = match test_sidecar_mock_state().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    guard.is_some()
}

#[cfg(test)]
pub(crate) async fn clear_test_sidecar_pool_for_host() {
    let mut pool = local_stt_sidecar_pool().lock().await;
    pool.active = None;
}

#[cfg(test)]
pub(crate) fn recorded_test_sidecar_ops_for_host() -> Vec<String> {
    let guard = match test_sidecar_mock_state().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    guard
        .as_ref()
        .map(|mock| mock.seen_ops.clone())
        .unwrap_or_default()
}

#[cfg(test)]
fn test_sidecar_mock_take(op: &str) -> Option<Result<LocalSttResponse, LocalError>> {
    let guard = match test_sidecar_mock_state().lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let mut guard = guard;
    let mock = guard.as_mut()?;
    mock.seen_ops.push(op.to_owned());
    if mock.sidecar_unavailable_remaining > 0 {
        mock.sidecar_unavailable_remaining -= 1;
        return Some(Err(LocalError::SidecarUnavailable(
            "mock sidecar exited".into(),
        )));
    }
    if let Some(error) = mock.fail_error.clone() {
        return Some(Err(LocalError::CommandFailed(error)));
    }
    Some(Ok(match op {
        "preload" | "load_model" => LocalSttResponse::Status(LocalSttStatus {
            warm: true,
            loaded_model: None,
            backend: Some("mock_sidecar".into()),
            accelerator: LocalSttAccelerator::Auto,
            device: None,
            profile: LocalSttProfile::Fast,
            idle_unload_after_ms: local_stt_idle_unload_after_ms_for_engine(DEFAULT_LOCAL_ENGINE),
        }),
        "transcribe" => {
            LocalSttResponse::Transcription(super::local_sidecar_protocol::LocalSttTranscription {
                text: mock
                    .transcript
                    .clone()
                    .unwrap_or_else(|| "mock sidecar transcript".into()),
                backend: "mock_sidecar".into(),
            })
        }
        _ => LocalSttResponse::Ack(super::local_sidecar_protocol::LocalSttAck {
            accepted: true,
            message: None,
        }),
    }))
}

async fn send_sidecar_request(request: LocalSttRequest) -> Result<LocalSttResponse, LocalError> {
    #[cfg(test)]
    {
        let op_name = match &request {
            LocalSttRequest::Status => "status",
            LocalSttRequest::LoadModel { .. } => "load_model",
            LocalSttRequest::Preload { .. } => "preload",
            LocalSttRequest::Transcribe { .. } => "transcribe",
            LocalSttRequest::Cancel { .. } => "cancel",
            LocalSttRequest::Unload => "unload",
            LocalSttRequest::Shutdown => "shutdown",
        };
        for attempt in 0..2 {
            if let Some(response) = test_sidecar_mock_take(op_name) {
                match response {
                    Err(LocalError::SidecarUnavailable(_)) if attempt == 0 => continue,
                    other => return other,
                }
            } else {
                break;
            }
        }

        if !matches!(
            env::var("MUNDUS_TEST_ALLOW_REAL_LOCAL_STT_SIDECAR").as_deref(),
            Ok("1")
        ) {
            return Err(LocalError::SidecarUnavailable(
                "real local STT sidecar is disabled in unit tests".into(),
            ));
        }
    }

    let mut pool = local_stt_sidecar_pool().lock().await;
    let request_clone = request.clone();
    for attempt in 0..2 {
        if pool.active.is_none() {
            pool.active = Some(LocalSttSidecarClient::spawn().await?);
        }

        let response = {
            let client = pool.active.as_mut().expect("sidecar client initialized");
            client
                .request(if attempt == 0 {
                    request.clone()
                } else {
                    request_clone.clone()
                })
                .await
        };
        match response {
            Ok(value) => return Ok(value),
            Err(LocalError::SidecarUnavailable(_)) if attempt == 0 => {
                pool.active = None;
            }
            Err(error) => return Err(error),
        }
    }
    Err(LocalError::SidecarUnavailable(
        "sidecar request retry budget exhausted".into(),
    ))
}
