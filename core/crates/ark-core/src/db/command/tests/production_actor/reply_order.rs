use super::*;
use crate::db::command::executor::actor::{ActorShared, Publish, WriterTask};
use ark_store::writer::SqliteWriter;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::oneshot;

struct ReplyOrderNoop;

impl WriterTask for ReplyOrderNoop {
    fn run(
        self: Box<Self>,
        _writer: &mut SqliteWriter,
        _on_error: &mut dyn FnMut(&DbError),
    ) -> (Result<(), DbError>, Publish) {
        (Ok(()), Box::new(|| {}))
    }

    fn cancel_before_start(self: Box<Self>) {}

    fn receiver_closed(&self) -> bool {
        false
    }
}

struct ReplyOrderProbe {
    shared: Arc<ActorShared>,
    admitted: oneshot::Sender<Result<(), DbError>>,
}

impl WriterTask for ReplyOrderProbe {
    fn run(
        self: Box<Self>,
        _writer: &mut SqliteWriter,
        _on_error: &mut dyn FnMut(&DbError),
    ) -> (Result<(), DbError>, Publish) {
        let Self { shared, admitted } = *self;
        let publish: Publish = Box::new(move || {
            let next = shared.admit(
                CommandClass::Interactive,
                &CommandSource::Engine,
                1,
                Box::new(ReplyOrderNoop),
            );
            let _ = admitted.send(next);
        });
        (Ok(()), publish)
    }

    fn cancel_before_start(self: Box<Self>) {
        let _ = self.admitted.send(Err(DbError::CancelledBeforeStart));
    }

    fn receiver_closed(&self) -> bool {
        self.admitted.is_closed()
    }
}

#[tokio::test]
async fn production_actor_reply_is_published_after_maintenance_release() {
    let directory = tempfile::tempdir().unwrap();
    let db_path = directory
        .path()
        .join("actor.db")
        .to_string_lossy()
        .into_owned();
    let (shared, mut owner, ready) =
        ActorShared::spawn(db_path, "actor-test-device".into(), actor_test_limits()).unwrap();
    ready.recv().unwrap().unwrap();

    let (admitted_tx, admitted_rx) = oneshot::channel();
    shared
        .admit(
            CommandClass::Maintenance,
            &CommandSource::Engine,
            1,
            Box::new(ReplyOrderProbe {
                shared: Arc::clone(&shared),
                admitted: admitted_tx,
            }),
        )
        .unwrap();
    let admitted = tokio::time::timeout(Duration::from_secs(10), admitted_rx)
        .await
        .expect("maintenance reply was never published")
        .expect("maintenance reply sender was dropped");

    owner
        .shutdown(Instant::now() + Duration::from_secs(2))
        .await
        .unwrap();
    assert!(
        admitted.is_ok(),
        "a command submitted right after a maintenance reply was refused: {admitted:?}"
    );
}
