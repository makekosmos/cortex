use super::*;

struct StdCommand;
impl StdCommand {
    fn new(program: &str) -> std::process::Command {
        assert_eq!(program, "git");
        isolated_std_git()
    }
}

fn temp_repo(dir: &tempfile::TempDir) -> PathBuf {
    let repo = dir.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    assert!(StdCommand::new("git")
        .args(["init", "--quiet"])
        .current_dir(&repo)
        .status()
        .unwrap()
        .success());
    repo
}

fn configured_repo(dir: &tempfile::TempDir) -> PathBuf {
    let repo = temp_repo(dir);
    for args in [
        vec!["config", "user.email", "daedalus@test.invalid"],
        vec!["config", "user.name", "Daedalus Test"],
    ] {
        assert!(StdCommand::new("git")
            .args(args)
            .current_dir(&repo)
            .status()
            .unwrap()
            .success());
    }
    repo
}

fn commit_file(repo: &Path, name: &str, contents: &str, message: &str) {
    std::fs::write(repo.join(name), contents).unwrap();
    assert!(StdCommand::new("git")
        .args(["add", name])
        .current_dir(repo)
        .status()
        .unwrap()
        .success());
    assert!(StdCommand::new("git")
        .args(["commit", "-m", message])
        .current_dir(repo)
        .status()
        .unwrap()
        .success());
}

fn fake_app_server_env() {
    let script = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/daedalus-fake-app-server.mjs")
        .canonicalize()
        .unwrap();
    let script_arg = script
        .to_string_lossy()
        .trim_start_matches("\\\\?\\")
        .replace('\\', "/");
    std::env::set_var("DAEDALUS_FAKE_APP_SERVER_EXE", "node");
    std::env::set_var("DAEDALUS_FAKE_APP_SERVER_SCRIPT", &script_arg);
}

fn clear_fake_app_server_env() {
    std::env::remove_var("DAEDALUS_FAKE_APP_SERVER_EXE");
    std::env::remove_var("DAEDALUS_FAKE_APP_SERVER_SCRIPT");
}

// `cargo test` runs every agents test in one process. These two tests
// used to set and clear the process environment independently, so one
// test's cleanup made the other's restore spawn `codex` (absent on the
// runner) and mark the session failed. nextest does not share a process,
// which is why the suite passes locally. The guard clears the variables
// before the lock is released, including on panic.
fn fake_app_server_lock() -> &'static tokio::sync::Mutex<()> {
    static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    &LOCK
}

struct FakeAppServerEnv;

impl FakeAppServerEnv {
    fn arm() -> Self {
        fake_app_server_env();
        Self
    }
}

impl Drop for FakeAppServerEnv {
    fn drop(&mut self) {
        clear_fake_app_server_env();
    }
}

#[test]
fn git_fixture_rejects_a_cwd_that_would_fall_back_to_the_outer_repo() {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    assert!(StdCommand::new("git")
        .args(["init", "--quiet"])
        .current_dir(&repo)
        .status()
        .unwrap()
        .success());
    let nested = repo.join("nested-fixture");
    std::fs::create_dir_all(&nested).unwrap();

    // Regression: 2026-08-15. A failed fixture init must never let Git
    // discover the implementation repository and mutate it.
    let error = git_cwd_is_isolated(&nested).unwrap_err();
    assert!(
        error.to_string().contains("escaped requested fixture")
            || error.to_string().contains("not a git")
    );
    dir.close().unwrap();
}

#[test]
fn slug_is_git_safe() {
    assert_eq!(prompt_slug(" Fix: login!!! "), "fix-login");
}
#[test]
fn mode_mapping_is_explicit() {
    assert_eq!(
        mode_params("auto-review"),
        ("workspace-write", "on-request", "auto_review")
    );
    assert_eq!(
        mode_params("full-access"),
        ("danger-full-access", "never", "user")
    );
    assert_eq!(
        turn_policy("default")["sandboxPolicy"]["type"],
        "workspaceWrite"
    );
    assert_eq!(
        turn_policy("auto-review")["approvalsReviewer"],
        "auto_review"
    );
    assert_eq!(turn_policy("full-access")["approvalPolicy"], "never");
    assert_eq!(
        turn_policy("full-access")["sandboxPolicy"]["type"],
        "dangerFullAccess"
    );
}

#[test]
fn full_access_consent_rejects_every_changed_binding_and_expiry() {
    let binding = FullAccessConsentBinding {
        package_id: "daedalus".into(),
        package_version: "1.2.3".into(),
        project_id: "project".into(),
        project_path: "C:\\project".into(),
        mode: "full-access".into(),
        model: Some("gpt-test".into()),
        prompt_hash: "prompt-hash".into(),
        connection_id: Some(7),
    };
    let now = Instant::now();
    let changed = [
        FullAccessConsentBinding {
            package_id: "other-package".into(),
            ..binding.clone()
        },
        FullAccessConsentBinding {
            package_version: "other-version".into(),
            ..binding.clone()
        },
        FullAccessConsentBinding {
            project_id: "other-project".into(),
            ..binding.clone()
        },
        FullAccessConsentBinding {
            model: Some("other-model".into()),
            ..binding.clone()
        },
        FullAccessConsentBinding {
            prompt_hash: "other-prompt".into(),
            ..binding.clone()
        },
        FullAccessConsentBinding {
            connection_id: Some(8),
            ..binding.clone()
        },
    ];
    for changed_binding in changed {
        let mut registry = FullAccessConsentRegistry::default();
        let issued = registry.issue_at(binding.clone(), now);
        let request_id = issued["request_id"].as_str().unwrap();
        let token = registry
            .approve_at(request_id, true, Some(7), now)
            .unwrap()
            .unwrap();
        assert!(registry.consume_at(&token, &changed_binding, now).is_err());
    }

    let mut registry = FullAccessConsentRegistry::default();
    let issued = registry.issue_at(binding.clone(), now);
    let request_id = issued["request_id"].as_str().unwrap();
    assert!(registry.approve_at(request_id, true, Some(8), now).is_err());
    let denied_token = registry.pending.get(request_id).unwrap().token.clone();
    assert!(registry
        .approve_at(request_id, false, Some(7), now)
        .unwrap()
        .is_none());
    assert!(registry.consume_at(&denied_token, &binding, now).is_err());

    let mut registry = FullAccessConsentRegistry::default();
    let issued = registry.issue_at(binding.clone(), now);
    let request_id = issued["request_id"].as_str().unwrap();
    let token = registry
        .approve_at(request_id, true, Some(7), now)
        .unwrap()
        .unwrap();
    assert!(registry
        .consume_at(&token, &binding, now + FULL_ACCESS_CONSENT_TTL)
        .unwrap_err()
        .to_string()
        .contains("expired"));
}

#[tokio::test]
async fn full_access_consent_is_desktop_bound_single_use_and_audited() {
    let dir = tempfile::tempdir().unwrap();
    let repo = temp_repo(&dir);
    let service = AgentsService::new(dir.path()).unwrap();
    let project = service.add_project(repo.to_str().unwrap()).await.unwrap();
    let mut request = json!({
        "package_id": "com.kosmos.daedalus",
        "package_version": "1.2.3",
        "project_id": project["id"],
        "prompt": "do not store this prompt",
        "mode": "full-access",
        "model": "gpt-test"
    });

    assert!(service
        .issue_full_access_consent(
            request.clone(),
            &crate::engine_dispatch::DispatchClient::default()
        )
        .is_err());
    assert!(service
        .create_session({
            request["full_access_confirmed"] = json!(true);
            request.clone()
        })
        .await
        .is_err());

    let issued = service
        .issue_full_access_consent(
            request.clone(),
            &crate::engine_dispatch::DispatchClient {
                desktop_authorized: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(issued.get("consent_nonce").is_none());
    let request_id = issued["request_id"].as_str().unwrap();
    let approved = service
        .approve_full_access_consent(
            json!({"request_id": request_id, "approved": true}),
            &crate::engine_dispatch::DispatchClient {
                desktop_authorized: true,
                ..Default::default()
            },
        )
        .unwrap();
    let nonce = approved["consent_nonce"].as_str().unwrap().to_string();
    assert!(service
        .approve_full_access_consent(
            json!({"request_id": request_id, "approved": true}),
            &crate::engine_dispatch::DispatchClient {
                desktop_authorized: true,
                ..Default::default()
            },
        )
        .is_err());
    request["consent_nonce"] = json!(nonce);
    request["package_version"] = json!("forged");
    assert!(service.create_session(request.clone()).await.is_err());
    request["package_version"] = json!("1.2.3");
    assert!(service.create_session(request).await.is_err());

    let audit: Vec<(String, String)> = service
        .db()
        .prepare("SELECT event,result FROM security_audit ORDER BY id")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        audit,
        vec![
            ("consent_issuance".into(), "denied".into()),
            ("consent_consume".into(), "denied".into()),
            ("consent_issuance".into(), "issued".into()),
            ("consent_approval".into(), "accepted".into()),
            ("consent_approval".into(), "denied".into()),
            ("consent_mismatch".into(), "denied".into()),
            ("consent_replay".into(), "denied".into()),
        ]
    );
    let prompt_count: i64 = service
        .db()
        .query_row(
            "SELECT COUNT(*) FROM security_audit WHERE prompt_sha256=?1",
            ["do not store this prompt"],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(prompt_count, 0);
    drop(service);
    dir.close().unwrap();
}

#[tokio::test]
async fn full_access_consent_fails_closed_when_audit_storage_fails() {
    let dir = tempfile::tempdir().unwrap();
    let repo = temp_repo(&dir);
    let service = AgentsService::new(dir.path()).unwrap();
    let project = service.add_project(repo.to_str().unwrap()).await.unwrap();
    service
        .db()
        .execute("DROP TABLE security_audit", [])
        .unwrap();

    let error = service
        .issue_full_access_consent(
            json!({
                "package_id": "com.kosmos.daedalus",
                "package_version": "1.2.3",
                "project_id": project["id"],
                "prompt": "test",
                "mode": "full-access"
            }),
            &crate::engine_dispatch::DispatchClient {
                desktop_authorized: true,
                ..Default::default()
            },
        )
        .unwrap_err();
    assert!(error.to_string().contains("security audit failed"));
    assert!(service
        .full_access_consents
        .lock()
        .unwrap()
        .pending
        .is_empty());
    drop(service);
    dir.close().unwrap();
}

#[test]
fn timeline_paginates() {
    let dir = tempfile::tempdir().unwrap();
    let service = AgentsService::new(dir.path()).unwrap();
    for index in 0..3 {
        service
            .append_timeline("s", "message", json!({"index":index}), false)
            .unwrap();
    }
    let first = service
        .timeline(json!({"session_id":"s","limit":2}))
        .unwrap();
    assert_eq!(first["events"].as_array().unwrap().len(), 2);
    let cursor = first["nextCursor"].as_i64().unwrap();
    let second = service
        .timeline(json!({"session_id":"s","limit":2,"cursor":cursor}))
        .unwrap();
    assert_eq!(second["events"].as_array().unwrap().len(), 1);
    drop(service);
    dir.close().unwrap();
}
#[test]
fn output_is_utf8_truncated() {
    let value = "я".repeat(MAX_OUTPUT_BYTES);
    let out = truncate_utf8(value, MAX_OUTPUT_BYTES);
    assert!(out.len() <= MAX_OUTPUT_BYTES);
    assert!(std::str::from_utf8(out.as_bytes()).is_ok());
}
#[test]
fn untracked_text_has_unified_patch() {
    let patch = untracked_patch("src\\new.txt", "one\ntwo\n");
    assert!(patch.contains("+++ b/src/new.txt"));
    assert!(patch.contains("+two"));
}
#[test]
fn status_entries_consume_rename_origin_field() {
    // `git status --porcelain=v1 -z` emits staged renames as
    // `XY <to>\0<from>\0`; the <from> field is not a record.
    let status = "R  renamed.txt\0tracked.txt\0 M modified.txt\0?? new.txt\0\0";
    let mut changed = std::collections::BTreeMap::new();
    collect_status_entries(status, &mut changed);
    assert_eq!(changed.get("renamed.txt").unwrap(), "R ");
    assert_eq!(changed.get("modified.txt").unwrap(), " M");
    assert_eq!(changed.get("new.txt").unwrap(), "??");
    assert!(!changed.contains_key("tracked.txt"));
    assert!(!changed.contains_key("acked.txt"));
    assert_eq!(changed.len(), 3);
}
#[test]
fn persistence_round_trip_keeps_pending_approval() {
    let dir = tempfile::tempdir().unwrap();
    let service = AgentsService::new(dir.path()).unwrap();
    let approval = service
        .save_approval(
            "session",
            json!(7),
            "item/fileChange/requestApproval",
            json!({"reason":"test"}),
        )
        .unwrap();
    drop(service);
    let reopened = AgentsService::new(dir.path()).unwrap();
    let pending = reopened.pending_approvals().unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, approval.id);
    drop(reopened);
    dir.close().unwrap();
}
#[tokio::test]
async fn dirty_base_does_not_leak_into_worktree() {
    let dir = tempfile::tempdir().unwrap();
    let repo = configured_repo(&dir);
    commit_file(&repo, "tracked.txt", "base", "base");
    std::fs::write(repo.join("tracked.txt"), "dirty").unwrap();
    let worktree = dir.path().join("worktree");
    git_status(
        &repo,
        &[
            "worktree",
            "add",
            "-b",
            "codex/test-12345678",
            path_str(&worktree).unwrap(),
            "HEAD",
        ],
    )
    .await
    .unwrap();
    let second = dir.path().join("worktree-2");
    git_status(
        &repo,
        &[
            "worktree",
            "add",
            "-b",
            "codex/test-87654321",
            path_str(&second).unwrap(),
            "HEAD",
        ],
    )
    .await
    .unwrap();
    assert_eq!(
        std::fs::read_to_string(worktree.join("tracked.txt")).unwrap(),
        "base"
    );
    assert_eq!(
        std::fs::read_to_string(second.join("tracked.txt")).unwrap(),
        "base"
    );
    let service = AgentsService::new(dir.path()).unwrap();
    let timestamp = now();
    service
        .db()
        .execute(
            "INSERT INTO projects(id,name,path,created_at) VALUES('p','repo',?1,?2)",
            params![repo.to_string_lossy(), timestamp],
        )
        .unwrap();
    service
        .db()
        .execute(
            concat!(
                "INSERT INTO sessions(id,project_id,title,prompt,mode,status,branch,",
                "worktree_path,base_commit,created_at,updated_at,archived_at) VALUES('s','p',",
                "'t','p','default','archived','codex/test-12345678',?1,'HEAD',?2,?2,?2)"
            ),
            params![worktree.to_string_lossy(), timestamp],
        )
        .unwrap();
    service.remove_worktree("s").await.unwrap();
    assert!(!worktree.exists());
    drop(service);
    dir.close().unwrap();
}

#[tokio::test]
async fn lifecycle_lock_serializes_same_session() {
    let dir = tempfile::tempdir().unwrap();
    let service = AgentsService::new(dir.path()).unwrap();
    let first = service.lifecycle_lock("session");
    let held = first.lock().await;
    let second = service.lifecycle_lock("session");

    assert!(
        tokio::time::timeout(Duration::from_millis(20), second.lock())
            .await
            .is_err()
    );
    drop(held);
    let _released = tokio::time::timeout(Duration::from_secs(60), second.lock())
        .await
        .unwrap();
    drop(service);
    dir.close().unwrap();
}

#[tokio::test]
async fn stale_runtime_generation_cannot_remove_current_runtime() {
    let dir = tempfile::tempdir().unwrap();
    let service = AgentsService::new(dir.path()).unwrap();
    let (tx, _rx) = mpsc::channel(1);
    service.runtimes().insert(
        "session".into(),
        RuntimeHandle {
            tx,
            generation: 2,
            task: tokio::spawn(async {}),
        },
    );

    assert!(!service.runtime_is_current("session", 1));
    service.remove_runtime("session", 1);
    assert!(service.runtime_is_current("session", 2));
    drop(service);
    dir.close().unwrap();
}

#[tokio::test]
async fn app_server_events_are_normalized_and_streams_are_coalesced() {
    let dir = tempfile::tempdir().unwrap();
    let service = AgentsService::new(dir.path()).unwrap();
    let active_turn = tokio::sync::Mutex::new(None);
    for delta in ["Привет, ", "мир"] {
        handle_app_message(
            &service,
            "session",
            0,
            &active_turn,
            json!({"method":"item/agentMessage/delta","params":{"delta":delta}}),
        )
        .await;
    }
    handle_app_message(
        &service,
        "session",
        0,
        &active_turn,
        json!({"method":"item/completed","params":{"item":{"id":"item-1"}}}),
    )
    .await;
    handle_app_message(
        &service,
        "session",
        0,
        &active_turn,
        json!({"id":9,"method":"item/fileChange/requestApproval","params":{"reason":"test"}}),
    )
    .await;
    let page = service
        .timeline(json!({"session_id":"session","limit":20}))
        .unwrap();
    let events = page["events"].as_array().unwrap();
    let message = events
        .iter()
        .find(|event| event["kind"] == "message_delta")
        .unwrap();
    assert_eq!(message["payload"]["text"], "Привет, мир");
    assert_eq!(service.pending_approvals().unwrap().len(), 1);
    drop(service);
    dir.close().unwrap();
}

#[tokio::test]
async fn diff_contains_committed_uncommitted_and_untracked_text() {
    let dir = tempfile::tempdir().unwrap();
    let repo = configured_repo(&dir);
    commit_file(&repo, "tracked.txt", "base\n", "base");
    let base = git_output(&repo, &["rev-parse", "HEAD"])
        .await
        .unwrap()
        .trim()
        .to_string();
    let worktree = dir.path().join("worktree");
    git_status(
        &repo,
        &[
            "worktree",
            "add",
            "-b",
            "codex/diff-12345678",
            path_str(&worktree).unwrap(),
            &base,
        ],
    )
    .await
    .unwrap();
    std::fs::write(worktree.join("committed.txt"), "committed\n").unwrap();
    git_status(&worktree, &["add", "committed.txt"])
        .await
        .unwrap();
    git_status(&worktree, &["commit", "-m", "agent commit"])
        .await
        .unwrap();
    std::fs::write(worktree.join("tracked.txt"), "working\n").unwrap();
    std::fs::write(worktree.join("untracked.txt"), "untracked\n").unwrap();

    let service = AgentsService::new(dir.path()).unwrap();
    let timestamp = now();
    service
        .db()
        .execute(
            concat!(
                "INSERT INTO sessions(id,project_id,title,prompt,mode,status,branch,",
                "worktree_path,base_commit,created_at,updated_at) VALUES('s','p','t','p',",
                "'default','running','b',?1,?2,?3,?3)"
            ),
            params![worktree.to_string_lossy(), base, timestamp],
        )
        .unwrap();
    let diff = service.diff("s").await.unwrap();
    let unified = diff["unifiedDiff"].as_str().unwrap();
    assert!(unified.contains("committed"));
    assert!(unified.contains("working"));
    assert!(unified.contains("untracked"));
    service
        .db()
        .execute(
            "UPDATE sessions SET status='archived',archived_at=?2 WHERE id=?1",
            params!["s", now()],
        )
        .unwrap();
    assert!(service.remove_worktree("s").await.is_err());
    assert!(worktree.exists());
    drop(service);
    dir.close().unwrap();
}

#[tokio::test]
#[ignore = "requires installed and authenticated Codex CLI"]
async fn real_codex_smoke_creates_file() {
    assert_eq!(
        std::env::var("DAEDALUS_REAL_CODEX_SMOKE").as_deref(),
        Ok("1"),
        "set DAEDALUS_REAL_CODEX_SMOKE=1 explicitly"
    );
    let dir = tempfile::tempdir().unwrap();
    let repo = configured_repo(&dir);
    commit_file(&repo, "README.md", "Daedalus smoke\n", "base");

    let service = AgentsService::new(dir.path()).unwrap();
    let project = service.add_project(path_str(&repo).unwrap()).await.unwrap();
    let session = service
        .create_session(json!({
            "project_id": project["id"],
            "prompt": concat!(
                "Создай файл daedalus-real-smoke.txt с единственной строкой ok. Не изменяй ",
                "другие файлы.",
            ),
            "mode": "default"
        }))
        .await
        .unwrap();
    let session_id = session["id"].as_str().unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(180);
    loop {
        for approval in service.pending_approvals().unwrap() {
            if approval.session_id == session_id {
                service
                    .respond_approval(json!({"approval_id":approval.id,"decision":"accept"}))
                    .await
                    .unwrap();
            }
        }
        let current = service.get_session(session_id).unwrap();
        if matches!(
            current.status.as_str(),
            "completed" | "failed" | "interrupted"
        ) {
            assert_eq!(current.status, "completed");
            assert_eq!(
                std::fs::read_to_string(
                    Path::new(&current.worktree_path).join("daedalus-real-smoke.txt")
                )
                .unwrap()
                .trim(),
                "ok"
            );
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "Codex smoke timed out"
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    let handles = service.runtimes().drain().collect::<Vec<_>>();
    for (id, handle) in handles {
        let _ = handle.stop(&id).await;
    }
    drop(service);
    dir.close().unwrap();
}

#[tokio::test]
async fn fake_app_server_recovers_session_and_expires_stale_approval() {
    let _lock = fake_app_server_lock().lock().await;
    let _env = FakeAppServerEnv::arm();

    let dir = tempfile::tempdir().unwrap();
    let repo = configured_repo(&dir);
    commit_file(&repo, "README.md", "fixture\n", "base");

    let service = AgentsService::new(dir.path()).unwrap();
    let project = service.add_project(path_str(&repo).unwrap()).await.unwrap();
    let session = service
        .create_session(
            json!({"project_id":project["id"],"prompt":"fake recovery","mode":"default"}),
        )
        .await
        .unwrap();
    let session_id = session["id"].as_str().unwrap().to_string();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while service.pending_approvals().unwrap().is_empty() {
        assert!(tokio::time::Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    service.stop_runtime(&session_id).await.unwrap();
    drop(service);

    let reopened = AgentsService::new(dir.path()).unwrap();
    reopened.restore_active_sessions().await;
    assert_eq!(
        reopened.get_session(&session_id).unwrap().status,
        "completed"
    );
    assert!(reopened.pending_approvals().unwrap().is_empty());
    reopened.send(&session_id, "follow-up").await.unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while reopened.pending_approvals().unwrap().is_empty() {
        assert!(tokio::time::Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    reopened.interrupt(&session_id).await.unwrap();
    assert_eq!(
        reopened.get_session(&session_id).unwrap().status,
        "interrupted"
    );
    assert!(reopened.pending_approvals().unwrap().is_empty());

    let unresponsive = reopened
        .create_session(json!({
            "project_id":project["id"],
            "prompt":"interrupt-unresponsive",
            "mode":"default"
        }))
        .await
        .unwrap();
    let unresponsive_id = unresponsive["id"].as_str().unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while reopened
        .get_session(unresponsive_id)
        .unwrap()
        .active_turn_id
        .is_none()
    {
        assert!(tokio::time::Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    reopened.interrupt(unresponsive_id).await.unwrap();
    assert_eq!(
        reopened.get_session(unresponsive_id).unwrap().status,
        "interrupted"
    );
    assert!(!reopened.runtimes().contains_key(unresponsive_id));

    let archived = reopened
        .create_session(json!({
            "project_id":project["id"],
            "prompt":"archive while waiting approval",
            "mode":"default"
        }))
        .await
        .unwrap();
    let archived_id = archived["id"].as_str().unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    while !reopened
        .pending_approvals()
        .unwrap()
        .iter()
        .any(|approval| approval.session_id == archived_id)
    {
        assert!(tokio::time::Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    reopened.archive(archived_id).await.unwrap();
    let archived = reopened.get_session(archived_id).unwrap();
    assert_eq!(archived.status, "archived");
    assert!(archived.active_turn_id.is_none());
    assert!(!reopened
        .pending_approvals()
        .unwrap()
        .iter()
        .any(|approval| approval.session_id == archived_id));
    reopened.shutdown().await;
    assert!(reopened.runtimes().is_empty());
    assert_eq!(Arc::strong_count(&reopened), 1);
    drop(reopened);
    dir.close().unwrap();
}

#[tokio::test]
async fn finished_runtime_registry_stays_bounded() {
    let dir = tempfile::tempdir().unwrap();
    let service = AgentsService::new(dir.path()).unwrap();

    // A handle whose task already completed is pruned by the next push,
    // so a long-lived service never accumulates dead JoinHandles.
    let mut done = tokio::spawn(async {});
    (&mut done).await.unwrap();
    let (tx, _rx) = mpsc::channel(1);
    service.runtimes().insert(
        "done".into(),
        RuntimeHandle {
            tx,
            generation: 1,
            task: done,
        },
    );
    service.remove_runtime("done", 1);

    let (tx, _rx) = mpsc::channel(1);
    let (park_tx, park_rx) = tokio::sync::oneshot::channel::<()>();
    service.runtimes().insert(
        "live".into(),
        RuntimeHandle {
            tx,
            generation: 1,
            task: tokio::spawn(async move {
                let _ = park_rx.await;
            }),
        },
    );
    service.remove_runtime("live", 1);
    assert_eq!(service.finished_tasks.lock().unwrap().len(), 1);

    let _ = park_tx.send(());
    service.shutdown().await;
    assert!(service.finished_tasks.lock().unwrap().is_empty());
    assert_eq!(Arc::strong_count(&service), 1);
    drop(service);
    dir.close().unwrap();
}

#[tokio::test]
async fn stopped_runtime_releases_its_service_arc() {
    // KOS-314 contract: the Shutdown ack precedes the runtime task's
    // teardown, so only a joined task proves the Arc<AgentsService> — and
    // the SQLite connection inside the data dir — is released.
    let _lock = fake_app_server_lock().lock().await;
    let _env = FakeAppServerEnv::arm();

    let dir = tempfile::tempdir().unwrap();
    let repo = configured_repo(&dir);
    commit_file(&repo, "README.md", "fixture\n", "base");
    let service = AgentsService::new(dir.path()).unwrap();
    let project = service.add_project(path_str(&repo).unwrap()).await.unwrap();
    let session = service
        .create_session(
            json!({"project_id":project["id"],"prompt":"arc contract","mode":"default"}),
        )
        .await
        .unwrap();
    let session_id = session["id"].as_str().unwrap().to_string();
    assert!(Arc::strong_count(&service) > 1);

    service.stop_runtime(&session_id).await.unwrap();
    assert_eq!(Arc::strong_count(&service), 1);
    drop(service);
    dir.close().unwrap();
}
