#![allow(clippy::unwrap_used)]
#![cfg(feature = "iroh-spike")]
#![allow(clippy::unwrap_used)]

//! Изолированный де-риск spike (НЕ интеграция с `iroh_transport.rs`): доказать,
//! что два `iroh::Endpoint` в одном процессе соединяются офлайн напрямую
//! (без relay/discovery/internet) на iroh 1.0.0, и зафиксировать точные
//! рабочие вызовы API.
//!
//! Рецепт офлайн loopback (см. `.agent/tasks/2026-06-16-iroh-transport/`):
//! - `Endpoint::builder(presets::Minimal)` — пресет НЕ добавляет address
//!   lookup (DNS/pkarr), только настраивает `crypto_provider`. Это даёт
//!   гарантированно офлайн-старт (в отличие от `presets::N0`/`N0DisableRelay`,
//!   которые всё равно конфигурируют DNS address lookup).
//! - `.relay_mode(RelayMode::Disabled)` — явно выключает relay (хотя
//!   `Minimal` и не включает его сам).
//! - `.alpns(vec![ARK_SYNC_ALPN.to_vec()])` — ALPN нужен на стороне, которая
//!   будет принимать соединение (accept проверяет ALPN из этого списка).
//! - `.bind().await` — биндит реальные UDP-сокеты.
//! - `endpoint.bound_sockets()` — единственный надёжный способ получить
//!   реальный локальный `SocketAddr` сразу после `bind()` без ожидания
//!   watcher/net_report (`endpoint.addr()` использует `watch_addr()` —
//!   асинхронный watcher, который в офлайн-режиме без net_report может не
//!   заполниться вовсе).
//! - `EndpointAddr::new(id).with_ip_addr(socket_addr)` — ручная сборка адреса
//!   пира для прямого коннекта, без relay url и без discovery.

use std::time::Duration;

use iroh::endpoint::presets;
use iroh::{Endpoint, EndpointAddr, RelayMode};

const ARK_SYNC_ALPN: &[u8] = b"ark-sync/1";

#[tokio::test(flavor = "multi_thread")]
async fn iroh_offline_loopback_round_trip() {
    tokio::time::timeout(Duration::from_secs(10), run())
        .await
        .expect("iroh offline loopback smoke test timed out after 10s");
}

async fn run() {
    // --- A: будет принимать соединение ---
    let endpoint_a = Endpoint::builder(presets::Minimal)
        .relay_mode(RelayMode::Disabled)
        .alpns(vec![ARK_SYNC_ALPN.to_vec()])
        .bind()
        .await
        .expect("endpoint A bind");

    let endpoint_a_id = endpoint_a.id();
    let endpoint_a_sockets = endpoint_a.bound_sockets();
    // `bound_sockets()` отдаёт адрес как у сервера, забинженного на
    // unspecified-адресе (`0.0.0.0:port` / `[::]:port`) — это валидно для
    // `bind()`, но НЕ является адресом, на который можно реально открыть
    // соединение. Для loopback-теста нужно явно подставить `127.0.0.1` (IPv4
    // unspecified-запись), сохранив реальный забинженный порт.
    let endpoint_a_port = endpoint_a_sockets
        .iter()
        .find(|addr| addr.is_ipv4())
        .expect("endpoint A should have an IPv4 bound socket")
        .port();
    let endpoint_a_socket: std::net::SocketAddr =
        (std::net::Ipv4Addr::LOCALHOST, endpoint_a_port).into();

    let endpoint_a_addr = EndpointAddr::new(endpoint_a_id).with_ip_addr(endpoint_a_socket);
    eprintln!("[smoke] endpoint A id={endpoint_a_id} sockets={endpoint_a_sockets:?}");

    // --- B: инициатор соединения, без relay/discovery ---
    let endpoint_b = Endpoint::builder(presets::Minimal)
        .relay_mode(RelayMode::Disabled)
        .bind()
        .await
        .expect("endpoint B bind");

    // --- A: фоновый accept-loop ---
    let accept_task = tokio::spawn({
        let endpoint_a = endpoint_a.clone();
        async move {
            eprintln!("[smoke] A: waiting for incoming");
            let incoming = endpoint_a
                .accept()
                .await
                .expect("endpoint A: expected an incoming connection");
            eprintln!("[smoke] A: got incoming, awaiting Connecting");
            let conn = incoming.await.expect("endpoint A: connecting failed");
            eprintln!("[smoke] A: connection established");

            let (mut send, mut recv) = conn.accept_bi().await.expect("endpoint A: accept_bi");

            let mut buf = [0u8; 4];
            recv.read_exact(&mut buf)
                .await
                .expect("endpoint A: read ping");
            assert_eq!(&buf, b"ping", "endpoint A: unexpected payload from B");

            send.write_all(b"pong")
                .await
                .expect("endpoint A: write pong");
            send.finish().expect("endpoint A: finish send stream");

            conn.closed().await;
        }
    });

    // --- B: подключаемся напрямую к A по собранному EndpointAddr ---
    eprintln!("[smoke] B: connecting to A at {endpoint_a_addr:?}");
    let conn = endpoint_b
        .connect(endpoint_a_addr, ARK_SYNC_ALPN)
        .await
        .expect("endpoint B: direct offline connect to A failed");
    eprintln!("[smoke] B: connected");

    let (mut send, mut recv) = conn.open_bi().await.expect("endpoint B: open_bi");

    send.write_all(b"ping")
        .await
        .expect("endpoint B: write ping");
    send.finish().expect("endpoint B: finish send stream");

    let mut buf = [0u8; 4];
    recv.read_exact(&mut buf)
        .await
        .expect("endpoint B: read pong");
    assert_eq!(&buf, b"pong", "endpoint B: unexpected payload from A");

    conn.close(0u32.into(), b"bye");

    accept_task.await.expect("endpoint A accept task panicked");

    endpoint_a.close().await;
    endpoint_b.close().await;
}
