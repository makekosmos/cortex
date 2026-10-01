use super::connection_types::ConnectionLoopArgs;
use super::ops::send_message;
use super::*;

pub(super) async fn run(args: ConnectionLoopArgs) -> Result<(), WsServerError> {
    let ConnectionLoopArgs {
        mut sink,
        mut stream,
        command_bus,
        pomodoro_host,
        dictation_host,
        ark_host,
        agent_events,
        correlation_id,
        client_id,
        desktop_authority,
        snapshots,
        grants,
        shutdown,
        dispatcher,
        hello,
    } = args;
    let mut bus_rx = command_bus.subscribe();
    let mut pomo_rx = pomodoro_host.subscribe();
    let mut dict_rx = dictation_host.subscribe();
    let mut ark_evt_rx = ark_host.subscribe_events();
    let mut agents_rx = agent_events.subscribe();

    loop {
        tokio::select! {
            evt = bus_rx.recv() => {
                match evt {
                    Ok(CommandBusEvent::Changed(list)) => {
                        let payload = serde_json::json!({
                            "event": "commands_changed",
                            "commands": list,
                        });
                        if send_message(&mut sink, Message::Text(payload.to_string()), &shutdown).await.is_err() {
                            break;
                        }
                    }
                    Ok(CommandBusEvent::Invoked { id, params }) => {
                        let payload = serde_json::json!({
                            "event": "command_invoked",
                            "id": id,
                            "params": params,
                        });
                        if send_message(&mut sink, Message::Text(payload.to_string()), &shutdown).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        // Resubscribe-friendly: drop the lagged event, continue.
                        continue;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }

            aevt = ark_evt_rx.recv() => {
                match aevt {
                    Ok((_name, payload)) => {
                        if send_message(&mut sink, Message::Text(payload.to_string()), &shutdown).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }

            // Daedalus events are already flat wire envelopes.
            agent_evt = agents_rx.recv() => {
                match agent_evt {
                    Ok(payload) => {
                        if send_message(&mut sink, Message::Text(payload.to_string()), &shutdown).await.is_err() { break; }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }

            // 2a'. Pomodoro events → forward as wire-formatted JSON.
            pevt = pomo_rx.recv() => {
                match pevt {
                    Ok(payload) => {
                        if send_message(&mut sink, Message::Text(payload.to_string()), &shutdown).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }

            // 2a''. Dictation events (state_changed / transcript / config_changed) → forward.
            devt = dict_rx.recv() => {
                match devt {
                    Ok(payload) => {
                        if send_message(&mut sink, Message::Text(payload.to_string()), &shutdown).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }

            // 2b. Incoming: WS frame from client → dispatch.
            frame = stream.next() => {
                let frame = match frame {
                    Some(f) => f,
                    None => break,
                };

                let text = match frame {
                    Ok(Message::Text(t)) => t,
                    Ok(Message::Close(_)) => break,
                    Ok(Message::Ping(p)) => {
                        if send_message(&mut sink, Message::Pong(p), &shutdown).await.is_err() {
                            break;
                        }
                        continue;
                    }
                    Ok(_) => continue, // binary/pong — игнор
                    Err(error) => {
                        tracing::warn!(client_id, error = %error, "Engine WebSocket receive failed");
                        break;
                    }
                };

                let request_value: serde_json::Value = match serde_json::from_str(&text) {
                    Ok(value) => value,
                    Err(error) => {
                        let malformed = Message::Text(serde_json::json!({
                            "ok": false,
                            "error": format!("malformed JSON: {error}"),
                        }).to_string());
                        if send_message(&mut sink, malformed, &shutdown).await.is_err() {
                            break;
                        }
                        continue;
                    }
                };
                if request_value.get("operation").and_then(Value::as_str)
                    == Some("desktop.authority.bind")
                {
                    let params = request_value.get("params").unwrap_or(&Value::Null);
                    let bound = desktop_authority
                        .bind_request(params, hello.pid.unwrap_or_default(), client_id)
                        .is_ok();

                    let response = serde_json::json!({
                        "id": request_value.get("id").cloned().unwrap_or(Value::Null),
                        "ok": bound,
                        "data": if bound { serde_json::json!({ "ok": true }) } else { Value::Null },
                        "code": if bound { Value::Null } else { Value::String("DESKTOP_AUTHORITY_BIND_DENIED".into()) },
                        "error": if bound { Value::Null } else { Value::String("desktop authority denied".into()) },
                    });
                    if send_message(&mut sink, Message::Text(response.to_string()), &shutdown).await.is_err() {
                        break;
                    }
                    continue;
                }
                let request = match crate::engine_dispatch::DispatchRequest::from_wire(request_value) {
                    Ok(request) => request.with_client(crate::engine_dispatch::DispatchClient {
                        pid: hello.pid,
                        class: hello.client_class.clone(),
                        version: hello.client_version.clone(),
                        correlation_id: Some(correlation_id.as_ref().clone()),
                        connection_id: Some(client_id),
                        desktop_authorized: desktop_authority.authorize(client_id),
                    }),
                    Err(error) => {
                        let invalid = Message::Text(serde_json::json!({
                            "id": serde_json::Value::Null,
                            "ok": false,
                            "error": error.to_string(),
                        }).to_string());
                        if send_message(&mut sink, invalid, &shutdown).await.is_err() {
                            break;
                        }
                        continue;
                    }
                };
                let request_id_value = request.request_id.clone();
                let (result_sender, result_receiver) = tokio::sync::oneshot::channel();
                let (cancel_sender, cancel_receiver) = tokio::sync::oneshot::channel();
                let cancel = Arc::new(Mutex::new(Some(cancel_sender)));
                let permit = match shutdown.lifecycle.request_capacity.clone().try_acquire_owned() {
                    Ok(permit) => Arc::new(Mutex::new(Some(permit))),
                    Err(_) => {
                        let payload = serde_json::json!({"id": request_id_value, "ok": false, "error": "WS request capacity exhausted"});
                        if send_message(&mut sink, Message::Text(payload.to_string()), &shutdown).await.is_err() { break; }
                        continue;
                    }
                };
                let request_id = shutdown.lifecycle.next_request.fetch_add(1, Ordering::Relaxed);
                let task_shutdown = shutdown.clone();
                let task_dispatcher = dispatcher.clone();
                let task = shutdown.install_request(request_id, permit.clone(), cancel.clone(), move |start_receiver| {
                    tokio::spawn(async move {
                        if start_receiver.await.is_err() {
                            return;
                        }
                        let _permit = permit.lock().unwrap_or_else(|p| p.into_inner()).take();
                        // Rejection metadata for the Engine log (KOS-298) —
                        // captured before `request` is consumed by dispatch.
                        // Metadata only, never payload values.
                        let client_class = request.client.class.clone();
                        let operation = request.operation.as_str().to_owned();
                        let type_id = crate::observability::app_rpc_type_id(&request.params)
                            .map(str::to_owned);
                        let dispatch = task_dispatcher.dispatch(request);
                        let result = tokio::select! {
                            _ = task_shutdown.cancelled() => Err("server shutting down".to_string()),
                            _ = cancel_receiver => Err("request cancelled".to_string()),
                            result = tokio::time::timeout(task_shutdown.response_deadline(), dispatch) => match result {
                                Ok(Ok(value)) => Ok(value),
                                Ok(Err(error)) => {
                                    crate::observability::log_app_rpc_rejection(
                                        client_class.as_deref().unwrap_or("-"),
                                        &operation,
                                        type_id.as_deref(),
                                        &error.to_string(),
                                    );
                                    Err(error.to_string())
                                }
                                Err(_) => {
                                    crate::observability::log_app_rpc_rejection(
                                        client_class.as_deref().unwrap_or("-"),
                                        &operation,
                                        type_id.as_deref(),
                                        "timeout",
                                    );
                                    Err("dispatch timed out".to_string())
                                }
                            },
                        };
                        let _ = result_sender.send(result);
                    })
                });
                if !task {
                    break;
                }
                let mut result_receiver = result_receiver;
                let payload = loop {
                    tokio::select! {
                        result = &mut result_receiver => {
                            break match result {
                                Ok(Ok(payload)) => payload,
                                Ok(Err(error)) => serde_json::json!({"id": request_id_value, "ok": false, "error": error}),
                                Err(_) => serde_json::json!({"id": request_id_value, "ok": false, "error": "dispatch task failed"}),
                            };
                        }
                        _ = shutdown.cancelled() => {
                            shutdown.finish_request(request_id, true).await;
                            return Ok(());
                        }
                        frame = stream.next() => {
                            match frame {
                                Some(Ok(Message::Close(_))) | None => {
                                    shutdown.finish_request(request_id, true).await;
                                    return Ok(());
                                }
                                Some(Err(error)) => {
                                    tracing::warn!(client_id, error = %error, "Engine WebSocket receive failed during request");
                                    shutdown.finish_request(request_id, true).await;
                                    return Ok(());
                                }
                                Some(Ok(Message::Ping(p))) => {
                                    if send_message(&mut sink, Message::Pong(p), &shutdown).await.is_err() {
                                        shutdown.finish_request(request_id, true).await;
                                        return Ok(());
                                    }
                                }
                                Some(Ok(Message::Text(_)))
                                | Some(Ok(Message::Binary(_)))
                                | Some(Ok(Message::Pong(_)))
                                | Some(Ok(Message::Frame(_))) => {
                                    let busy = serde_json::json!({"id": serde_json::Value::Null, "ok": false, "error": "WS request busy"});
                                    if send_message(&mut sink, Message::Text(busy.to_string()), &shutdown).await.is_err() {
                                        shutdown.finish_request(request_id, true).await;
                                        return Ok(());
                                    }
                                }
                            }
                        }
                    }
                };
                shutdown.finish_request(request_id, false).await;
                if send_message(&mut sink, Message::Text(payload.to_string()), &shutdown).await.is_err() {
                    break;
                }
            }
        }
    }

    let grant_owner = desktop_authority
        .owner(client_id)
        .map(|(session_id, generation)| GrantOwner {
            session_id,
            generation,
            connection_id: client_id,
        });
    snapshots.close_owner(&format!("desktop-connection-{client_id}"));
    if let Some(owner) = grant_owner {
        grants.close_owner(owner);
    }
    desktop_authority.disconnect(client_id);
    Ok(())
}
