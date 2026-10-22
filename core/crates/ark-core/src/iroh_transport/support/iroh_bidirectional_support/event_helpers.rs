use std::time::Duration;

use tokio::sync::mpsc;

use ark_core::sync_transport::TransportEvent;
use ark_sync_contracts::wire::LanSyncMessage;

/// Generous hang guard for event drains — the awaited message is the real
/// condition; the cap only bounds a wedged transport under load (KOS-308).
pub const EVENT_GUARD: Duration = Duration::from_secs(60);

/// Вспомогательная функция: дренирует события из `rx` до тех пор, пока не
/// найдёт `MessageReceived` с нужным условием, либо не истечёт таймаут.
/// Пропускает `Connected`/`Disconnected` и `MessageReceived` других типов.
pub async fn find_message<F>(
    rx: &mut mpsc::UnboundedReceiver<TransportEvent>,
    timeout: Duration,
    predicate: F,
) -> Option<(String, LanSyncMessage)>
where
    F: Fn(&str, &LanSyncMessage) -> bool,
{
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return None;
        }
        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Some(
                TransportEvent::MessageReceived {
                    from_device_id,
                    msg,
                }
                | TransportEvent::MessageReceivedFromTransport {
                    from_device_id,
                    msg,
                    ..
                },
            )) => {
                if predicate(&from_device_id, &msg) {
                    return Some((from_device_id, msg));
                }
            }
            Ok(Some(_)) => {}
            Ok(None) => return None, // канал закрыт
            Err(_) => return None,   // таймаут
        }
    }
}
