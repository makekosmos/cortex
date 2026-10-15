use std::panic::{catch_unwind, AssertUnwindSafe};

use super::{handle_request, service_fixture, with_conn, with_write_tx, Request};

async fn initialized_fixture() -> super::ServiceFixture {
    let fixture = service_fixture();
    handle_request(
        &fixture.state,
        Request::Init {
            db_path: fixture.db_path().to_string_lossy().into_owned(),
        },
    )
    .await
    .unwrap();
    with_conn(&fixture.state, |conn| {
        conn.execute_batch(
            "CREATE TABLE tx_parent (id INTEGER PRIMARY KEY);
             CREATE TABLE tx_child (
                 parent_id INTEGER REFERENCES tx_parent(id) DEFERRABLE INITIALLY DEFERRED
             );",
        )
        .map_err(|error| error.to_string())
    })
    .unwrap();
    fixture
}

#[tokio::test]
async fn write_tx_rolls_back_when_deferred_foreign_key_commit_fails() {
    let fixture = initialized_fixture().await;
    let error = with_write_tx(&fixture.state, |conn| {
        conn.execute("INSERT INTO tx_child(parent_id) VALUES (404)", [])
            .map_err(|error| error.to_string())?;
        Ok(())
    })
    .unwrap_err();
    assert!(error.contains("FOREIGN KEY constraint failed"), "{error}");

    let conn = crate::service::runtime::get_shared_conn(&fixture.state).unwrap();
    let guard = conn.lock().unwrap_or_else(|error| error.into_inner());
    assert!(guard.is_autocommit());
    let child_count: i64 = guard
        .query_row("SELECT COUNT(*) FROM tx_child", [], |row| row.get(0))
        .unwrap();
    assert_eq!(child_count, 0);
    drop(guard);

    with_write_tx(&fixture.state, |conn| {
        conn.execute("INSERT INTO tx_parent(id) VALUES (1)", [])
            .map_err(|error| error.to_string())?;
        conn.execute("INSERT INTO tx_child(parent_id) VALUES (1)", [])
            .map_err(|error| error.to_string())?;
        Ok(())
    })
    .unwrap();
}

#[tokio::test]
async fn write_tx_rolls_back_callback_error() {
    let fixture = initialized_fixture().await;
    let result: Result<(), String> = with_write_tx(&fixture.state, |conn| {
        conn.execute("INSERT INTO tx_parent(id) VALUES (2)", [])
            .map_err(|error| error.to_string())?;
        Err("callback failed".to_string())
    });
    assert_eq!(result.unwrap_err(), "callback failed");

    let conn = crate::service::runtime::get_shared_conn(&fixture.state).unwrap();
    let guard = conn.lock().unwrap_or_else(|error| error.into_inner());
    assert!(guard.is_autocommit());
    let parent_count: i64 = guard
        .query_row("SELECT COUNT(*) FROM tx_parent", [], |row| row.get(0))
        .unwrap();
    assert_eq!(parent_count, 0);
}

#[tokio::test]
async fn write_tx_rolls_back_during_unwind() {
    let fixture = initialized_fixture().await;
    let panic = catch_unwind(AssertUnwindSafe(|| {
        let _ = with_write_tx(&fixture.state, |conn| -> Result<(), String> {
            conn.execute("INSERT INTO tx_parent(id) VALUES (3)", [])
                .map_err(|error| error.to_string())?;
            panic!("abort transaction closure");
        });
    }));
    assert!(panic.is_err());

    let conn = crate::service::runtime::get_shared_conn(&fixture.state).unwrap();
    let guard = conn.lock().unwrap_or_else(|error| error.into_inner());
    assert!(guard.is_autocommit());
    let parent_count: i64 = guard
        .query_row("SELECT COUNT(*) FROM tx_parent", [], |row| row.get(0))
        .unwrap();
    assert_eq!(parent_count, 0);
}
