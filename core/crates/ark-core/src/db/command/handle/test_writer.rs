use super::*;

pub(super) async fn spawn_test_store_writer<T, F>(
    connection: Arc<Mutex<Connection>>,
    _admission: executor::AdmissionPermit,
    operation: F,
) -> Result<T, DbError>
where
    T: Send + 'static,
    F: FnOnce(&mut Connection) -> Result<T, ark_store::writer::TransactionFailure<String>>
        + Send
        + 'static,
{
    tokio::task::spawn_blocking(move || {
        let _admission = _admission;
        let mut connection = connection.lock().unwrap_or_else(|error| error.into_inner());
        operation(&mut connection).map_err(map_transaction_failure)
    })
    .await
    .map_err(|error| DbError::Storage(error.to_string()))?
}

fn map_transaction_failure(failure: ark_store::writer::TransactionFailure<String>) -> DbError {
    match failure {
        ark_store::writer::TransactionFailure::Begin(error) => DbError::from_sqlite(error),
        ark_store::writer::TransactionFailure::Operation(error) => {
            DbError::from_application_error(error)
        }
        ark_store::writer::TransactionFailure::Commit(error) => {
            DbError::CommitFailed(error.to_string())
        }
    }
}
