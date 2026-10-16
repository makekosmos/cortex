use super::super::{CommandClass, CommandContext, CommandSource, DbError};
use rusqlite::{Connection, Transaction, TransactionBehavior};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::time::Instant;
use tokio::sync::oneshot;

use super::super::{DbCommand, DbMaintenanceCommand};
use super::AdmissionLimits;

mod runtime;
use runtime::{actor_loop, panic_message};

pub(in crate::db::command) trait WriterTask: Send {
    fn run(self: Box<Self>, connection: &mut Connection) -> Result<(), DbError>;
    fn cancel_before_start(self: Box<Self>);
    fn receiver_closed(&self) -> bool;
}

pub(in crate::db::command) struct TypedCommand<C: DbCommand> {
    pub context: CommandContext,
    pub command: C,
    pub reply: oneshot::Sender<Result<C::Output, DbError>>,
}

impl<C: DbCommand> WriterTask for TypedCommand<C> {
    fn run(self: Box<Self>, connection: &mut Connection) -> Result<(), DbError> {
        let Self {
            context,
            command,
            reply,
        } = *self;
        let result: Result<C::Output, DbError> = (|| {
            let transaction =
                Transaction::new_unchecked(connection, TransactionBehavior::Immediate)
                    .map_err(|error| DbError::Storage(error.to_string()))?;
            let output = command.execute(&context, &transaction)?;
            transaction
                .commit()
                .map_err(|error| DbError::Storage(error.to_string()))?;
            Ok(output)
        })();
        match result {
            Ok(output) => {
                let _ = reply.send(Ok(output));
                Ok(())
            }
            Err(error) => {
                let _ = reply.send(Err(error.clone()));
                Err(error)
            }
        }
    }

    fn cancel_before_start(self: Box<Self>) {
        let _ = self.reply.send(Err(DbError::CancelledBeforeStart));
    }

    fn receiver_closed(&self) -> bool {
        self.reply.is_closed()
    }
}

pub(in crate::db::command) struct TypedMaintenance<C: DbMaintenanceCommand> {
    pub command: C,
    pub reply: oneshot::Sender<Result<C::Output, DbError>>,
}

impl<C: DbMaintenanceCommand> WriterTask for TypedMaintenance<C> {
    fn run(self: Box<Self>, connection: &mut Connection) -> Result<(), DbError> {
        let Self { command, reply } = *self;
        match command.execute(connection) {
            Ok(output) => {
                let _ = reply.send(Ok(output));
                Ok(())
            }
            Err(error) => {
                let _ = reply.send(Err(error.clone()));
                Err(error)
            }
        }
    }

    fn cancel_before_start(self: Box<Self>) {
        let _ = self.reply.send(Err(DbError::CancelledBeforeStart));
    }

    fn receiver_closed(&self) -> bool {
        self.reply.is_closed()
    }
}

struct Slot {
    task: Box<dyn WriterTask>,
    source: CommandSource,
    class: CommandClass,
    bytes: usize,
    ticket: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Lifecycle {
    Opening,
    Migrating,
    Ready,
    Draining,
    Recovering,
    Failed,
    Closed,
}

struct SourceQueue {
    pending: VecDeque<usize>,
    commands: usize,
    bytes: usize,
}

struct Ledger {
    slots: Vec<Option<Slot>>,
    free: Vec<usize>,
    by_source: HashMap<CommandSource, SourceQueue>,
    commands: usize,
    bytes: usize,
    next_ticket: u64,
    scheduler_cursor: usize,
    maintenance_active: bool,
    lifecycle: Lifecycle,
    drain_deadline: Option<Instant>,
    failure: Option<String>,
}

pub(in crate::db::command) struct ActorShared {
    pub(in crate::db::command) path: String,
    limits: AdmissionLimits,
    ledger: Mutex<Ledger>,
    wake: Condvar,
    pub(in crate::db::command) readers: RwLock<()>,
    closed: Mutex<Option<oneshot::Sender<()>>>,
    generation: std::sync::atomic::AtomicU64,
    sequence: std::sync::atomic::AtomicU64,
}

pub(in crate::db::command) struct ActorOwner {
    pub(in crate::db::command) shared: Arc<ActorShared>,
    join: Option<std::thread::JoinHandle<()>>,
    closed: Option<oneshot::Receiver<()>>,
}

pub(in crate::db::command) struct ReadPermit {
    shared: Arc<ActorShared>,
    source: CommandSource,
    bytes: usize,
}

type SpawnedActor = (
    Arc<ActorShared>,
    ActorOwner,
    std::sync::mpsc::Receiver<Result<(), DbError>>,
);

impl ActorShared {
    pub(in crate::db::command) fn spawn(
        path: String,
        bootstrap_device_id: String,
        limits: AdmissionLimits,
    ) -> Result<SpawnedActor, std::io::Error> {
        let slots = (0..limits.max_commands).map(|_| None).collect();
        let free = (0..limits.max_commands).rev().collect();
        let (closed_tx, closed_rx) = oneshot::channel();
        let shared = Arc::new(Self {
            path: path.clone(),
            limits,
            ledger: Mutex::new(Ledger {
                slots,
                free,
                by_source: HashMap::with_capacity(limits.max_sources),
                commands: 0,
                bytes: 0,
                next_ticket: 0,
                scheduler_cursor: 0,
                maintenance_active: false,
                lifecycle: Lifecycle::Opening,
                drain_deadline: None,
                failure: None,
            }),
            wake: Condvar::new(),
            readers: RwLock::new(()),
            closed: Mutex::new(Some(closed_tx)),
            generation: std::sync::atomic::AtomicU64::new(1),
            sequence: std::sync::atomic::AtomicU64::new(0),
        });
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let actor_shared = Arc::clone(&shared);
        let join = std::thread::Builder::new()
            .name("ark-db-writer".to_string())
            .spawn(move || actor_loop(actor_shared, bootstrap_device_id, ready_tx))?;
        let owner = ActorOwner {
            shared: Arc::clone(&shared),
            join: Some(join),
            closed: Some(closed_rx),
        };
        Ok((shared, owner, ready_rx))
    }

    pub(in crate::db::command) fn admit(
        &self,
        class: CommandClass,
        source: &CommandSource,
        bytes: usize,
        task: Box<dyn WriterTask>,
    ) -> Result<(), DbError> {
        if bytes == 0 || bytes > self.limits.max_command_bytes {
            return Err(DbError::TooLarge);
        }
        let mut ledger = self
            .ledger
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        match ledger.lifecycle {
            Lifecycle::Ready => {}
            Lifecycle::Opening | Lifecycle::Migrating => return Err(DbError::Unavailable),
            Lifecycle::Recovering => return Err(DbError::RecoveryRequired),
            Lifecycle::Failed => {
                return Err(DbError::Storage(
                    ledger
                        .failure
                        .clone()
                        .unwrap_or_else(|| "database actor failed".into()),
                ))
            }
            Lifecycle::Draining | Lifecycle::Closed => return Err(DbError::Unavailable),
        }
        let source_queue = ledger.by_source.get(source);
        let next_source_commands = source_queue.map_or(1, |queue| queue.commands + 1);
        let next_source_bytes = source_queue.map_or(bytes, |queue| queue.bytes + bytes);
        if ledger.free.is_empty()
            || (source_queue.is_none() && ledger.by_source.len() >= self.limits.max_sources)
            || next_source_commands > self.limits.max_commands_per_source
            || next_source_bytes > self.limits.max_bytes_per_source
            || ledger.commands + 1 > self.limits.max_commands
            || ledger.bytes + bytes > self.limits.max_bytes
            || (class != CommandClass::Interactive
                && (ledger.commands + 1
                    > self.limits.max_commands - self.limits.reserved_interactive_commands
                    || ledger.bytes + bytes
                        > self.limits.max_bytes - self.limits.reserved_interactive_bytes))
            || (class == CommandClass::Backfill
                && (ledger.commands + 1
                    > self.limits.max_commands
                        - self.limits.reserved_interactive_commands
                        - self.limits.reserved_sync_live_commands
                    || ledger.bytes + bytes
                        > self.limits.max_bytes
                            - self.limits.reserved_interactive_bytes
                            - self.limits.reserved_sync_live_bytes))
        {
            return Err(DbError::Busy);
        }
        let index = ledger.free.pop().expect("free slot checked above");
        let ticket = ledger.next_ticket;
        ledger.next_ticket = ledger.next_ticket.wrapping_add(1);
        ledger.slots[index] = Some(Slot {
            task,
            source: source.clone(),
            class,
            bytes,
            ticket,
        });
        let queue = ledger
            .by_source
            .entry(source.clone())
            .or_insert_with(|| SourceQueue {
                pending: VecDeque::with_capacity(self.limits.max_commands),
                commands: 0,
                bytes: 0,
            });
        queue.pending.push_back(index);
        queue.commands += 1;
        queue.bytes += bytes;
        ledger.commands += 1;
        ledger.bytes += bytes;
        self.wake.notify_one();
        Ok(())
    }

    pub(in crate::db::command) fn admit_read(
        self: &Arc<Self>,
        class: CommandClass,
        source: &CommandSource,
        bytes: usize,
    ) -> Result<ReadPermit, DbError> {
        if bytes == 0 || bytes > self.limits.max_command_bytes {
            return Err(DbError::TooLarge);
        }
        let mut ledger = self
            .ledger
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        match ledger.lifecycle {
            Lifecycle::Ready => {}
            Lifecycle::Opening | Lifecycle::Migrating | Lifecycle::Draining | Lifecycle::Closed => {
                return Err(DbError::Unavailable)
            }
            Lifecycle::Recovering => return Err(DbError::RecoveryRequired),
            Lifecycle::Failed => {
                return Err(DbError::Storage(
                    ledger
                        .failure
                        .clone()
                        .unwrap_or_else(|| "database actor failed".into()),
                ))
            }
        }
        if ledger.maintenance_active
            || ledger
                .slots
                .iter()
                .flatten()
                .any(|slot| slot.class == CommandClass::Maintenance)
        {
            return Err(DbError::Busy);
        }
        let source_queue = ledger.by_source.get(source);
        let next_source_commands = source_queue.map_or(1, |queue| queue.commands + 1);
        let next_source_bytes = source_queue.map_or(bytes, |queue| queue.bytes + bytes);
        if (source_queue.is_none() && ledger.by_source.len() >= self.limits.max_sources)
            || next_source_commands > self.limits.max_commands_per_source
            || next_source_bytes > self.limits.max_bytes_per_source
            || ledger.commands + 1 > self.limits.max_commands
            || ledger.bytes + bytes > self.limits.max_bytes
            || (class != CommandClass::Interactive
                && (ledger.commands + 1
                    > self.limits.max_commands - self.limits.reserved_interactive_commands
                    || ledger.bytes + bytes
                        > self.limits.max_bytes - self.limits.reserved_interactive_bytes))
            || (class == CommandClass::Backfill
                && (ledger.commands + 1
                    > self.limits.max_commands
                        - self.limits.reserved_interactive_commands
                        - self.limits.reserved_sync_live_commands
                    || ledger.bytes + bytes
                        > self.limits.max_bytes
                            - self.limits.reserved_interactive_bytes
                            - self.limits.reserved_sync_live_bytes))
        {
            return Err(DbError::Busy);
        }
        let queue = ledger
            .by_source
            .entry(source.clone())
            .or_insert_with(|| SourceQueue {
                pending: VecDeque::with_capacity(self.limits.max_commands),
                commands: 0,
                bytes: 0,
            });
        queue.commands += 1;
        queue.bytes += bytes;
        ledger.commands += 1;
        ledger.bytes += bytes;
        Ok(ReadPermit {
            shared: Arc::clone(self),
            source: source.clone(),
            bytes,
        })
    }

    pub(in crate::db::command) fn begin_shutdown(&self, deadline: Instant) {
        let mut ledger = self
            .ledger
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if matches!(ledger.lifecycle, Lifecycle::Closed | Lifecycle::Failed) {
            return;
        }
        ledger.lifecycle = Lifecycle::Draining;
        ledger.drain_deadline = Some(deadline);
        self.wake.notify_all();
    }

    #[cfg(test)]
    pub(in crate::db::command) fn admitted_commands(&self) -> usize {
        self.ledger
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .commands
    }

    #[cfg(test)]
    pub(in crate::db::command) fn is_ready(&self) -> bool {
        self.ledger
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .lifecycle
            == Lifecycle::Ready
    }

    #[cfg(test)]
    pub(in crate::db::command) fn generation(&self) -> u64 {
        self.generation.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl Drop for ReadPermit {
    fn drop(&mut self) {
        let mut ledger = self
            .shared
            .ledger
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        ledger.commands -= 1;
        ledger.bytes -= self.bytes;
        if let Some(queue) = ledger.by_source.get_mut(&self.source) {
            queue.commands -= 1;
            queue.bytes -= self.bytes;
            if queue.commands == 0 {
                ledger.by_source.remove(&self.source);
            }
        }
        self.shared.wake.notify_all();
    }
}

impl ActorOwner {
    pub(in crate::db::command) async fn shutdown(
        &mut self,
        deadline: Instant,
    ) -> Result<(), DbError> {
        self.shared.begin_shutdown(deadline);
        let Some(closed) = self.closed.as_mut() else {
            return Ok(());
        };
        let remaining = deadline.saturating_duration_since(Instant::now());
        match tokio::time::timeout(remaining, &mut *closed).await {
            Ok(Ok(())) => {}
            Ok(Err(_)) => return Err(DbError::Unavailable),
            Err(_) => return Err(DbError::OutcomeUnknown),
        }
        self.closed.take();
        if let Some(join) = self.join.take() {
            tokio::task::spawn_blocking(move || join.join())
                .await
                .map_err(|error| DbError::Storage(error.to_string()))?
                .map_err(|panic| DbError::Storage(panic_message(&*panic)))?;
        }
        Ok(())
    }
}

impl Drop for ActorOwner {
    fn drop(&mut self) {
        self.shared.begin_shutdown(Instant::now());
        // Dropping JoinHandle detaches; the actor owns the connection and will
        // finish its current SQLite operation before observing this close.
        self.join.take();
    }
}
