async fn run_app_server(
    service: Arc<AgentsService>,
    session: Session,
    generation: u64,
    mut rx: mpsc::Receiver<AppCommand>,
    ready: tokio::sync::oneshot::Sender<Result<(), String>>,
) {
    let resumed = session.codex_thread_id.is_some();
    let result = start_app_server(&session).await;
    let mut startup = match result {
        Ok(parts) => parts,
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };
    if !service.runtime_is_current(&session.id, generation) {
        let result = startup
            .process_tree
            .terminate_and_wait(Duration::from_secs(5))
            .await
            .map(|_| ())
            .map_err(|error| error.to_string());
        let deadline = tokio::time::Instant::now() + Duration::from_secs(4);
        loop {
            match tokio::time::timeout_at(deadline, rx.recv()).await {
                Ok(Some(AppCommand::Shutdown(done))) => {
                    if let Some(done) = done {
                        let _ = done.send(result);
                    }
                    break;
                }
                Ok(Some(AppCommand::Interrupt { done, .. })) => {
                    let _ = done.send(Err("runtime was replaced during startup".into()));
                }
                Ok(Some(AppCommand::Approval { done, .. })) => {
                    let _ = done.send(Err("runtime was replaced during startup".into()));
                }
                Ok(Some(AppCommand::Send { .. })) => {}
                Ok(None) | Err(_) => break,
            }
        }
        let _ = ready.send(Err("runtime was replaced during startup".into()));
        return;
    }
    let next_id = AtomicI64::new(100);
    let mut pending_turn_requests = HashSet::<i64>::new();
    let mut pending_interrupt = None;
    let active_turn = Arc::new(tokio::sync::Mutex::new(startup.active_turn_id.take()));
    if resumed
        && matches!(
            &startup.status,
            SessionStatus::Completed | SessionStatus::Interrupted | SessionStatus::Failed
        )
    {
        let _ = service.expire_pending_approvals(&session.id, "app_server_restarted");
    }
    for message in startup.buffered.drain(..) {
        handle_app_message(&service, &session.id, generation, &active_turn, message).await;
    }
    if !resumed {
        let request_id = next_id.fetch_add(1, Ordering::Relaxed);
        if send_turn(
            &mut startup.stdin,
            request_id,
            &startup.thread_id,
            &session.prompt,
            session.model.as_deref(),
            &session.mode,
            None,
        )
        .await
        .is_err()
        {
            let _ = service.set_status(&session.id, SessionStatus::Failed);
            return;
        }
        pending_turn_requests.insert(request_id);
    }
    let _ = service.set_status(&session.id, startup.status);
    let active_turn_id = active_turn.lock().await.clone();
    let _ = service.update_codex_ids(
        &session.id,
        Some(&startup.thread_id),
        active_turn_id.as_deref(),
    );
    service.emit(
        "session_updated",
        &session.id,
        json!(service.get_session(&session.id).ok()),
    );
    let _ = ready.send(Ok(()));
    loop {
        tokio::select! {
            command = rx.recv() => match command {
                Some(AppCommand::Send { text }) => {
                    let turn_id = active_turn.lock().await.clone();
                    let request_id = next_id.fetch_add(1, Ordering::Relaxed);
                    let send = send_turn(
                        &mut startup.stdin,
                        request_id,
                        &startup.thread_id,
                        &text,
                        session.model.as_deref(),
                        &session.mode,
                        turn_id.as_deref(),
                    )
                    .await;
                    if let Err(error) = send {
                        let _ = service
                            .append_and_emit(&session.id, "error", json!({ "message": error }));
                    } else {
                        pending_turn_requests.insert(request_id);
                    }
                }
                Some(AppCommand::Interrupt { expected_turn_id, done }) => {
                    let current = active_turn.lock().await.clone();
                    if current.as_deref() != Some(expected_turn_id.as_str()) {
                        let _ = done.send(Err("active turn changed before interrupt".into()));
                    } else if pending_interrupt.is_some() {
                        let _ = done.send(Err("interrupt already pending".into()));
                    } else {
                        let request_id = next_id.fetch_add(1, Ordering::Relaxed);
                        let sent = send_rpc(
                            &mut startup.stdin,
                            request_id,
                            "turn/interrupt",
                            json!({
                                "threadId": startup.thread_id,
                                "turnId": expected_turn_id,
                            }))
                        .await;
                        match sent {
                            Ok(()) => {
                                pending_interrupt = Some((request_id, expected_turn_id, done));
                            }
                            Err(error) => {
                                let _ = done.send(Err(error));
                            }
                        }
                    }
                }
                Some(AppCommand::Approval { request_id, result, done }) => {
                    let result = write_json(
                        &mut startup.stdin,
                        &json!({ "id": request_id, "result": result }))
                    .await;
                    let _ = done.send(result);
                }
                Some(AppCommand::Shutdown(done)) => {
                    if let Some((_, _, interrupt_done)) = pending_interrupt.take() {
                        let _ = interrupt_done.send(Err("runtime stopped".into()));
                    }
                    let result = startup
                        .process_tree
                        .terminate_and_wait(Duration::from_secs(5))
                        .await
                        .map(|_| ())
                        .map_err(|error| error.to_string());
                    if let Some(done) = done {
                        let _ = done.send(result);
                    }
                    break;
                }
                None => {
                    let _ = startup
                        .process_tree
                        .terminate_and_wait(Duration::from_secs(5))
                        .await;
                    break;
                }
            },
            line = startup.lines.next_line() => match line {
                Ok(Some(line)) => if let Ok(message) = serde_json::from_str::<Value>(&line) {
                    if !service.runtime_is_current(&session.id, generation) {
                        continue;
                    }
                    if let Some((request_id, _, _)) = pending_interrupt.as_ref() {
                        let answered = message.get("id").and_then(Value::as_i64)
                            == Some(*request_id);
                        if answered && message.get("error").is_some() {
                            if let Some((_, _, done)) = pending_interrupt.take() {
                                let _ = done.send(Err(message["error"].to_string()));
                            }
                        }
                    }
                    let request_id = message
                        .get("id")
                        .and_then(Value::as_i64)
                        .filter(|id| pending_turn_requests.remove(id));
                    if let Some(id) = request_id {
                        if let Some(error) = message.get("error") {
                            let _ = service.set_status(&session.id, SessionStatus::Failed);
                            let _ = service.append_and_emit(
                                &session.id,
                                "error",
                                json!({ "requestId": id, "message": error }));
                            service.emit(
                                "session_updated",
                                &session.id,
                                json!(service.get_session(&session.id).ok()),
                            );
                        } else if let Some(turn_id) = message
                            .pointer("/result/turn/id")
                            .and_then(Value::as_str)
                        {
                            *active_turn.lock().await = Some(turn_id.to_string());
                            let _ =
                                service.update_codex_ids(&session.id, None, Some(turn_id));
                        }
                    }
                    let completed =
                        message.get("method").and_then(Value::as_str) == Some("turn/completed");
                    let completed_id = message
                        .pointer("/params/turn/id")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                    let completed_status = message
                        .pointer("/params/turn/status")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                    handle_app_message(&service, &session.id, generation, &active_turn, message)
                        .await;
                    if completed {
                        if let Some((_, expected, done)) = pending_interrupt.take() {
                            let acknowledged = completed_id.as_deref()
                                == Some(expected.as_str())
                                && completed_status.as_deref() == Some("interrupted");
                            let result = if acknowledged {
                                Ok(())
                            } else {
                                Err("active turn did not acknowledge interrupt".into())
                            };
                            let _ = done.send(result);
                        }
                    }
                },
                Ok(None) | Err(_) => {
                    if let Some((_, _, done)) = pending_interrupt.take() {
                        let _ = done.send(Err("app-server pipe closed".into()));
                    }
                    let active = service
                        .get_session(&session.id)
                        .map(|current| {
                            matches!(
                                current.status.as_str(),
                                "starting" | "running" | "waiting_approval" | "interrupting"
                            )
                        })
                        .unwrap_or(true);
                    let should_fail =
                        service.runtime_is_current(&session.id, generation) && active;
                    if should_fail {
                        let _ = service.set_status(&session.id, SessionStatus::Failed);
                        service.emit(
                            "session_updated",
                            &session.id,
                            json!(service.get_session(&session.id).ok()),
                        );
                    }
                    break;
                }
            }
        }
    }
    service.remove_runtime(&session.id, generation);
}

async fn start_app_server(session: &Session) -> Result<AppServerStartup, String> {
    // codex is an npm `codex.cmd` shim on Windows: resolve it before setting
    // stdio, which a rebuilt command would drop.
    let mut command = process_tree::resolve_command(codex_command())
        .map_err(|e| format!("Не удалось запустить Codex CLI: {e}"))?;
    command
        .args(["app-server", "--stdio"])
        .current_dir(&session.worktree_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut process_tree = process_tree::ProcessTree::spawn(&mut command, 0)
        .await
        .map_err(|e| format!("Не удалось запустить Codex CLI: {e}"))?;
    let mut stdin = process_tree
        .child_mut()
        .stdin
        .take()
        .ok_or("Codex stdin недоступен")?;
    let stdout = process_tree
        .child_mut()
        .stdout
        .take()
        .ok_or("Codex stdout недоступен")?;
    let mut lines = BufReader::new(stdout).lines();
    let (init, mut buffered) = rpc_call(
        &mut stdin,
        &mut lines,
        1,
        "initialize",
        json!({
            "clientInfo": {
                "name": "daedalus",
                "title": "Mundus Daedalus",
                "version": "0.1.0",
            },
            "capabilities": { "experimentalApi": true },
        }),
    )
    .await?;
    if init.get("error").is_some() {
        return Err(format!("Codex initialize: {}", init["error"]));
    }
    write_json(&mut stdin, &json!({"method":"initialized","params":{}})).await?;
    let (sandbox, approval_policy, reviewer) = mode_params(&session.mode);
    let (method, params_value) = if let Some(thread_id) = &session.codex_thread_id {
        (
            "thread/resume",
            json!({
                "threadId": thread_id,
                "cwd": session.worktree_path,
                "model": session.model,
                "sandbox": sandbox,
                "approvalPolicy": approval_policy,
                "approvalsReviewer": reviewer,
            }),
        )
    } else {
        (
            "thread/start",
            json!({
                "cwd": session.worktree_path,
                "model": session.model,
                "sandbox": sandbox,
                "approvalPolicy": approval_policy,
                "approvalsReviewer": reviewer,
                "experimentalRawEvents": false,
            }),
        )
    };
    let (response, pending) = rpc_call(&mut stdin, &mut lines, 2, method, params_value).await?;
    buffered.extend(pending);
    let thread_id = response
        .pointer("/result/thread/id")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("Codex thread/start не вернул thread id: {response}"))?
        .to_string();
    let last_turn = response
        .pointer("/result/thread/turns")
        .and_then(Value::as_array)
        .and_then(|turns| turns.last());
    let active_turn_id = last_turn
        .filter(|turn| turn.get("status").and_then(Value::as_str) == Some("inProgress"))
        .and_then(|turn| turn.get("id").and_then(Value::as_str))
        .map(str::to_string);
    let status = if session.codex_thread_id.is_none() || active_turn_id.is_some() {
        SessionStatus::Running
    } else {
        match last_turn
            .and_then(|turn| turn.get("status"))
            .and_then(Value::as_str)
        {
            Some("failed") => SessionStatus::Failed,
            Some("interrupted") | Some("inProgress") => SessionStatus::Interrupted,
            _ => SessionStatus::Completed,
        }
    };
    Ok(AppServerStartup {
        process_tree,
        stdin,
        lines,
        thread_id,
        active_turn_id,
        status,
        buffered,
    })
}

async fn handle_app_message(
    service: &AgentsService,
    session_id: &str,
    generation: u64,
    active_turn: &tokio::sync::Mutex<Option<String>>,
    message: Value,
) {
    if generation != 0 && !service.runtime_is_current(session_id, generation) {
        return;
    }
    if let (Some(id), Some(method)) = (
        message.get("id"),
        message.get("method").and_then(Value::as_str),
    ) {
        let params_value = message.get("params").cloned().unwrap_or_else(|| json!({}));
        if method.contains("requestApproval") || method == "item/tool/requestUserInput" {
            if let Ok(approval) =
                service.save_approval(session_id, id.clone(), method, params_value.clone())
            {
                let kind = if method == "item/tool/requestUserInput" {
                    "question"
                } else {
                    "approval"
                };
                let _ = service.append_and_emit(
                    session_id,
                    kind,
                    json!({"approvalId":approval.id,"method":method,"params":params_value}),
                );
                service.emit("approval_requested", session_id, json!(approval));
            }
            return;
        }
    }
    let method = match message.get("method").and_then(Value::as_str) {
        Some(v) => v,
        None => return,
    };
    let params_value = message.get("params").cloned().unwrap_or_else(|| json!({}));
    if method == "serverRequest/resolved" {
        if let Some(request_id) = params_value.get("requestId") {
            let _ = service.resolve_server_request(session_id, request_id);
        }
        return;
    }
    let kind = match method {
        "turn/started" => {
            if let Some(turn_id) = params_value.pointer("/turn/id").and_then(Value::as_str) {
                *active_turn.lock().await = Some(turn_id.to_string());
                let _ = service.update_codex_ids(session_id, None, Some(turn_id));
            }
            let _ = service.set_status(session_id, SessionStatus::Running);
            "turn_started"
        }
        "turn/completed" => {
            *active_turn.lock().await = None;
            let status = match params_value.pointer("/turn/status").and_then(Value::as_str) {
                Some("failed") => SessionStatus::Failed,
                Some("interrupted") => SessionStatus::Interrupted,
                _ => SessionStatus::Completed,
            };
            let _ = service.set_status(session_id, status);
            let _ = service.update_codex_ids(session_id, None, None);
            "turn_completed"
        }
        "item/started" => "item_started",
        "item/completed" => "item_completed",
        "item/agentMessage/delta" => "message_delta",
        "item/commandExecution/outputDelta" => "command_output",
        "item/fileChange/outputDelta" | "item/fileChange/patchUpdated" => "file_change_delta",
        "item/reasoning/summaryTextDelta" => "reasoning_delta",
        "turn/plan/updated" | "turn/planUpdated" => "plan",
        "turn/diff/updated" => {
            service.emit("diff_updated", session_id, params_value.clone());
            "diff"
        }
        "error" => "error",
        other => {
            tracing::debug!(
                method = other,
                session_id,
                "unknown Codex app-server notification"
            );
            return;
        }
    };
    let _ = service.append_and_emit(session_id, kind, params_value);
    if method == "turn/completed" || method == "turn/started" {
        service.emit(
            "session_updated",
            session_id,
            json!(service.get_session(session_id).ok()),
        );
    }
}

async fn rpc_call(
    stdin: &mut tokio::process::ChildStdin,
    lines: &mut Lines<BufReader<ChildStdout>>,
    id: i64,
    method: &str,
    params_value: Value,
) -> Result<(Value, Vec<Value>), String> {
    send_rpc(stdin, id, method, params_value).await?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(45);
    let mut buffered = Vec::new();
    while let Some(line) = tokio::time::timeout_at(deadline, lines.next_line())
        .await
        .map_err(|_| format!("Codex {method}: timeout"))?
        .map_err(|e| e.to_string())?
    {
        let value: Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
        if value.get("id").and_then(Value::as_i64) == Some(id) {
            return Ok((value, buffered));
        }
        buffered.push(value);
    }
    Err("Codex app-server закрыл stdout".into())
}

async fn send_turn(
    stdin: &mut tokio::process::ChildStdin,
    id: i64,
    thread_id: &str,
    text: &str,
    model: Option<&str>,
    mode: &str,
    active_turn_id: Option<&str>,
) -> Result<(), String> {
    if let Some(turn_id) = active_turn_id {
        return send_rpc(
            stdin,
            id,
            "turn/steer",
            json!({
                "threadId": thread_id,
                "expectedTurnId": turn_id,
                "input": [{ "type": "text", "text": text }],
            }),
        )
        .await;
    }
    let policy = turn_policy(mode);
    send_rpc(
        stdin,
        id,
        "turn/start",
        json!({
            "threadId": thread_id,
            "input": [{ "type": "text", "text": text }],
            "model": model,
            "approvalPolicy": policy["approvalPolicy"],
            "sandboxPolicy": policy["sandboxPolicy"],
            "approvalsReviewer": policy["approvalsReviewer"],
        }),
    )
    .await
}

async fn send_rpc(
    stdin: &mut tokio::process::ChildStdin,
    id: i64,
    method: &str,
    params_value: Value,
) -> Result<(), String> {
    write_json(
        stdin,
        &json!({"id":id,"method":method,"params":params_value}),
    )
    .await
}
async fn write_json(stdin: &mut tokio::process::ChildStdin, value: &Value) -> Result<(), String> {
    stdin
        .write_all(format!("{value}\n").as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    stdin.flush().await.map_err(|e| e.to_string())
}

async fn codex_one_shot(method: &str, params_value: Value) -> Result<Value, String> {
    let mut command = process_tree::resolve_command(codex_command()).map_err(|e| e.to_string())?;
    let mut child = command
        .args(["app-server", "--stdio"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| e.to_string())?;
    let mut stdin = child.stdin.take().ok_or("Codex stdin недоступен")?;
    let stdout = child.stdout.take().ok_or("Codex stdout недоступен")?;
    let mut lines = BufReader::new(stdout).lines();
    let _ = rpc_call(
        &mut stdin,
        &mut lines,
        1,
        "initialize",
        json!({
            "clientInfo": { "name": "daedalus", "version": "0.1.0" },
            "capabilities": { "experimentalApi": true },
        }),
    )
    .await?;
    write_json(&mut stdin, &json!({"method":"initialized","params":{}})).await?;
    let (response, _) = rpc_call(&mut stdin, &mut lines, 2, method, params_value).await?;
    let _ = child.kill().await;
    response
        .get("result")
        .cloned()
        .ok_or_else(|| format!("Codex {method}: {response}"))
}

fn mode_params(mode: &str) -> (&'static str, &'static str, &'static str) {
    match mode {
        "auto-review" => ("workspace-write", "on-request", "auto_review"),
        "full-access" => ("danger-full-access", "never", "user"),
        _ => ("workspace-write", "on-request", "user"),
    }
}
fn turn_policy(mode: &str) -> Value {
    let (sandbox, approval_policy, reviewer) = mode_params(mode);
    let sandbox_policy = if sandbox == "danger-full-access" {
        json!({"type":"dangerFullAccess"})
    } else {
        json!({"type":"workspaceWrite","networkAccess":false})
    };
    json!({
        "approvalPolicy": approval_policy,
        "sandboxPolicy": sandbox_policy,
        "approvalsReviewer": reviewer,
    })
}
fn codex_command() -> Command {
    // Unit tests may run alongside other runtime tests that mutate the shared
    // MUNDUS_TEST_MODE environment variable. The fake app-server variables are
    // test-only and are the stable selector for this command override.
    if cfg!(test) || std::env::var("MUNDUS_TEST_MODE").as_deref() == Ok("1") {
        if let (Ok(executable), Ok(script)) = (
            std::env::var("DAEDALUS_FAKE_APP_SERVER_EXE"),
            std::env::var("DAEDALUS_FAKE_APP_SERVER_SCRIPT"),
        ) {
            let mut command = Command::new(executable);
            command.arg(script);
            return command;
        }
    }
    Command::new("codex")
}
fn now() -> String {
    Utc::now().to_rfc3339()
}

fn new_consent_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
fn canonical_project_path(project: &Project) -> Result<String, String> {
    Path::new(&project.path)
        .canonicalize()
        .map(|path| path.to_string_lossy().into_owned())
        .map_err(|error| format!("project path is unavailable: {error}"))
}
fn required_str(value: &Value, key: &str) -> Result<String, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|v| !v.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("missing '{key}'"))
}
fn path_str(path: &Path) -> Result<&str, String> {
    path.to_str().ok_or_else(|| "Путь не является UTF-8".into())
}
fn hex_hash(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}
fn prompt_slug(prompt: &str) -> String {
    let slug = prompt
        .chars()
        .flat_map(char::to_lowercase)
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>();
    let compact = slug
        .split('-')
        .filter(|p| !p.is_empty())
        .take(6)
        .collect::<Vec<_>>()
        .join("-");
    if compact.is_empty() {
        "task".into()
    } else {
        compact.chars().take(48).collect()
    }
}
fn truncate_utf8(mut text: String, max: usize) -> String {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text.truncate(end);
    text
}
fn untracked_patch(path: &str, content: &str) -> String {
    let normalized = path.replace('\\', "/");
    let line_count = content.lines().count().max(1);
    let body = content
        .lines()
        .map(|line| format!("+{line}\n"))
        .collect::<String>();
    format!(
        "\ndiff --git a/{normalized} b/{normalized}\nnew file mode 100644\n--- \
/dev/null\n+++ b/{normalized}\n@@ -0,0 +1,{line_count} @@\n{body}"
    )
}
fn git_dirty(path: &Path) -> bool {
    if git_cwd_is_isolated(path).is_err() {
        return false;
    }
    isolated_std_git()
        .args(["status", "--porcelain"])
        .current_dir(path)
        .output()
        .map(|o| !o.stdout.is_empty())
        .unwrap_or(false)
}
fn is_binary(path: &Path) -> bool {
    let mut bytes = [0_u8; 8192];
    std::fs::File::open(path)
        .and_then(|mut file| file.read(&mut bytes))
        .map(|count| bytes[..count].contains(&0))
        .unwrap_or(false)
}

async fn git_output(cwd: &Path, args: &[&str]) -> Result<String, String> {
    git_cwd_is_isolated(cwd)?;
    let output = isolated_async_git()
        .args(args)
        .current_dir(cwd)
        .output()
        .await
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn git_cwd_is_isolated(cwd: &Path) -> Result<(), String> {
    let requested = cwd
        .canonicalize()
        .map_err(|error| format!("Git working directory is unavailable: {error}"))?;
    let output = isolated_std_git()
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(&requested)
        .output()
        .map_err(|error| format!("Git repository probe failed: {error}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let root = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim())
        .canonicalize()
        .map_err(|error| format!("Git repository root is unavailable: {error}"))?;
    if requested != root {
        return Err(format!(
            "Git repository escaped requested fixture: {} -> {}",
            requested.display(),
            root.display()
        ));
    }
    Ok(())
}

fn git_repository_root(path: &Path) -> Result<PathBuf, String> {
    let requested = path
        .canonicalize()
        .map_err(|error| format!("Git repository path is unavailable: {error}"))?;
    let output = isolated_std_git()
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(&requested)
        .output()
        .map_err(|error| format!("Git repository probe failed: {error}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    PathBuf::from(String::from_utf8_lossy(&output.stdout).trim())
        .canonicalize()
        .map_err(|error| format!("Git repository root is unavailable: {error}"))
}

fn isolated_std_git() -> std::process::Command {
    let mut command = std::process::Command::new("git");
    for key in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_COMMON_DIR",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_GRAFT_FILE",
        "GIT_NAMESPACE",
    ] {
        command.env_remove(key);
    }
    command
}

fn isolated_async_git() -> Command {
    let mut command = Command::new("git");
    for key in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_COMMON_DIR",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_GRAFT_FILE",
        "GIT_NAMESPACE",
    ] {
        command.env_remove(key);
    }
    command
}
async fn git_status(cwd: &Path, args: &[&str]) -> Result<(), String> {
    git_output(cwd, args).await.map(|_| ())
}
async fn command_available(command: &str) -> bool {
    Command::new(command)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false)
}

fn session_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Session> {
    let worktree_path: String = row.get(8)?;
    Ok(Session {
        id: row.get(0)?,
        project_id: row.get(1)?,
        title: row.get(2)?,
        prompt: row.get(3)?,
        mode: row.get(4)?,
        model: row.get(5)?,
        status: row.get(6)?,
        branch: row.get(7)?,
        worktree_exists: Path::new(&worktree_path).is_dir(),
        worktree_path,
        base_commit: row.get(9)?,
        codex_thread_id: row.get(10)?,
        active_turn_id: row.get(11)?,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
        archived_at: row.get(14)?,
    })
}
fn timeline_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TimelineEvent> {
    let raw: String = row.get(3)?;
    Ok(TimelineEvent {
        id: row.get(0)?,
        session_id: row.get(1)?,
        kind: row.get(2)?,
        payload: serde_json::from_str(&raw).unwrap_or(Value::Null),
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
        truncated: row.get::<_, i64>(6)? != 0,
    })
}
fn approval_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Approval> {
    let request: String = row.get(2)?;
    let params_raw: String = row.get(4)?;
    let response: Option<String> = row.get(6)?;
    Ok(Approval {
        id: row.get(0)?,
        session_id: row.get(1)?,
        request_id: serde_json::from_str(&request).unwrap_or(Value::Null),
        method: row.get(3)?,
        params: serde_json::from_str(&params_raw).unwrap_or(Value::Null),
        status: row.get(5)?,
        response: response.and_then(|v| serde_json::from_str(&v).ok()),
        created_at: row.get(7)?,
        resolved_at: row.get(8)?,
    })
}
