use super::*;
use ark_store::writer::SqliteWriter;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;
use std::time::Instant;

pub(super) fn actor_loop(
    shared: Arc<ActorShared>,
    bootstrap_device_id: String,
    ready: std::sync::mpsc::Sender<Result<(), DbError>>,
) {
    let connection = (|| {
        let mut ledger = shared
            .ledger
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        ledger.lifecycle = Lifecycle::Migrating;
        drop(ledger);
        crate::db::recover_interrupted_restore(&shared.path).map_err(|error| {
            eprintln!("database restore recovery failed before startup: {error}");
            DbError::RecoveryRequired
        })?;
        let connection = ark_store::open::open_db(&shared.path).map_err(DbError::Storage)?;
        crate::db::init_schema(&connection).map_err(DbError::Storage)?;
        let mut writer = SqliteWriter::from_connection(connection);
        let backup_path = format!("{}.pre-core-task.bak", shared.path);
        ark_store::canonical_migrations::rewrite_legacy_tasks(
            &mut writer,
            &backup_path,
            &bootstrap_device_id,
        )
        .map_err(DbError::Storage)?;
        Ok(writer)
    })();
    let mut writer = match connection {
        Ok(writer) => {
            shared
                .ledger
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .lifecycle = Lifecycle::Ready;
            let _ = ready.send(Ok(()));
            writer
        }
        Err(error) => {
            let _ = ready.send(Err(error));
            let mut ledger = shared
                .ledger
                .lock()
                .unwrap_or_else(|lock_error| lock_error.into_inner());
            ledger.lifecycle = Lifecycle::Failed;
            ledger.failure = Some("database bootstrap failed".into());
            complete_close(&shared);
            return;
        }
    };

    loop {
        let slot_index = {
            let mut ledger = shared
                .ledger
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            loop {
                if ledger.lifecycle == Lifecycle::Draining
                    && ledger
                        .drain_deadline
                        .is_some_and(|deadline| Instant::now() >= deadline)
                {
                    cancel_all(&mut ledger);
                }
                if let Some(index) = take_next(&mut ledger) {
                    break Some(index);
                }
                if (ledger.lifecycle == Lifecycle::Draining
                    || ledger.lifecycle == Lifecycle::Failed)
                    && ledger.commands == 0
                {
                    ledger.lifecycle = Lifecycle::Closed;
                    break None;
                }
                ledger = shared
                    .wake
                    .wait(ledger)
                    .unwrap_or_else(|error| error.into_inner());
            }
        };
        let Some(index) = slot_index else { break };
        let Some(slot) = take_slot(&shared, index) else {
            continue;
        };
        if slot.task.receiver_closed() {
            slot.task.cancel_before_start();
            release_slot(&shared, index, slot.source, slot.class, slot.bytes);
            continue;
        }

        let (committed, publish) = if slot.class == CommandClass::Maintenance {
            let _exclusive = shared
                .readers
                .write()
                .unwrap_or_else(|error| error.into_inner());
            run_task(&shared, &mut writer, slot.task)
        } else {
            run_task(&shared, &mut writer, slot.task)
        };
        if committed {
            shared
                .sequence
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if slot.class == CommandClass::Maintenance {
                shared
                    .generation
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        }
        release_slot(&shared, index, slot.source, slot.class, slot.bytes);
        if let Some(publish) = publish {
            publish();
        }
    }
    drop(writer);
    complete_close(&shared);
}

fn run_task(
    shared: &ActorShared,
    writer: &mut SqliteWriter,
    task: Box<dyn WriterTask>,
) -> (bool, Option<Publish>) {
    let mut on_error = |error: &DbError| fail_closed_for_error(shared, error);
    match catch_unwind(AssertUnwindSafe(|| task.run(writer, &mut on_error))) {
        Ok((Ok(()), publish)) => (true, Some(publish)),
        Ok((Err(_error), publish)) => (false, Some(publish)),
        Err(panic) => {
            let mut ledger = shared
                .ledger
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            ledger.lifecycle = Lifecycle::Failed;
            ledger.failure = Some(format!(
                "database writer panicked: {}",
                panic_message(&*panic)
            ));
            drop(ledger);
            shared
                .generation
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            (false, None)
        }
    }
}

fn fail_closed_for_error(shared: &ActorShared, error: &DbError) {
    let failure = match error {
        DbError::RecoveryRequired => Some("database restore requires recovery".to_owned()),
        DbError::RestoreFailed(message) => Some(format!("database restore failed: {message}")),
        DbError::StorageUnhealthy(message) => Some(message.clone()),
        DbError::CommitFailed(message) => {
            Some(format!("database transaction commit failed: {message}"))
        }
        _ => None,
    };
    if let Some(failure) = failure {
        let mut ledger = shared
            .ledger
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        ledger.lifecycle = Lifecycle::Failed;
        ledger.failure = Some(failure);
    }
}

fn take_next(ledger: &mut Ledger) -> Option<usize> {
    if ledger.lifecycle == Lifecycle::Failed {
        cancel_all(ledger);
        return None;
    }
    let barrier_ticket = ledger
        .slots
        .iter()
        .flatten()
        .filter(|slot| slot.class == CommandClass::Maintenance)
        .map(|slot| slot.ticket)
        .min();
    let minimum_ticket = ledger.slots.iter().flatten().map(|slot| slot.ticket).min();
    if let (Some(barrier), Some(minimum)) = (barrier_ticket, minimum_ticket) {
        if minimum == barrier {
            if ledger.readers > 0 {
                return None;
            }
            return find_source_head(ledger, barrier);
        }
    }
    const SCHEDULE: [CommandClass; 7] = [
        CommandClass::Interactive,
        CommandClass::Interactive,
        CommandClass::Interactive,
        CommandClass::SyncLive,
        CommandClass::SyncLive,
        CommandClass::Backfill,
        CommandClass::Interactive,
    ];
    for offset in 0..SCHEDULE.len() {
        let class = SCHEDULE[(ledger.scheduler_cursor + offset) % SCHEDULE.len()];
        let before_barrier = barrier_ticket.unwrap_or(u64::MAX);
        let source_position = ledger.source_order.iter().position(|source| {
            ledger
                .by_source
                .get(source)
                .and_then(|queue| queue.pending.front())
                .and_then(|index| ledger.slots[*index].as_ref())
                .is_some_and(|slot| slot.class == class && slot.ticket < before_barrier)
        });
        if let Some(position) = source_position {
            let source = ledger
                .source_order
                .remove(position)
                .expect("source position came from queue");
            let index = ledger
                .by_source
                .get(&source)
                .and_then(|queue| queue.pending.front())
                .copied()
                .expect("eligible source has a pending command");
            ledger.source_order.push_back(source);
            ledger.scheduler_cursor = (ledger.scheduler_cursor + offset + 1) % SCHEDULE.len();
            return Some(index);
        }
    }
    None
}

fn find_source_head(ledger: &Ledger, ticket: u64) -> Option<usize> {
    ledger
        .by_source
        .values()
        .filter_map(|queue| queue.pending.front().copied())
        .find(|index| {
            ledger.slots[*index]
                .as_ref()
                .is_some_and(|slot| slot.ticket == ticket)
        })
}

fn take_slot(shared: &ActorShared, index: usize) -> Option<Slot> {
    let mut ledger = shared
        .ledger
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let slot = ledger.slots[index].take()?;
    if slot.class == CommandClass::Maintenance {
        ledger.maintenance_active = true;
    }
    if let Some(queue) = ledger.by_source.get_mut(&slot.source) {
        if queue.pending.front() == Some(&index) {
            queue.pending.pop_front();
        }
    }
    Some(slot)
}

fn release_slot(
    shared: &ActorShared,
    index: usize,
    source: CommandSource,
    class: CommandClass,
    bytes: usize,
) {
    let mut ledger = shared
        .ledger
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    ledger.free.push(index);
    if class == CommandClass::Maintenance {
        ledger.maintenance_active = false;
    }
    ledger.commands -= 1;
    ledger.bytes -= bytes;
    if let Some(queue) = ledger.by_source.get_mut(&source) {
        queue.commands -= 1;
        queue.bytes -= bytes;
        if queue.commands == 0 {
            ledger.by_source.remove(&source);
            ledger.source_order.retain(|queued| queued != &source);
        }
    }
    shared.wake.notify_all();
}

fn cancel_all(ledger: &mut Ledger) {
    let mut canceled = Vec::new();
    for (index, slot) in ledger.slots.iter_mut().enumerate() {
        if let Some(slot) = slot.take() {
            canceled.push((index, slot));
        }
    }
    for (index, slot) in canceled {
        slot.task.cancel_before_start();
        ledger.free.push(index);
        ledger.commands -= 1;
        ledger.bytes -= slot.bytes;
        if let Some(queue) = ledger.by_source.get_mut(&slot.source) {
            queue.commands -= 1;
            queue.bytes -= slot.bytes;
        }
    }
    ledger.by_source.retain(|_, queue| queue.commands > 0);
    ledger
        .source_order
        .retain(|source| ledger.by_source.contains_key(source));
}

fn complete_close(shared: &ActorShared) {
    let mut sender = shared
        .closed
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if let Some(sender) = sender.take() {
        let _ = sender.send(());
    }
}

pub(super) fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|message| (*message).to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "unknown database actor panic".into())
}
