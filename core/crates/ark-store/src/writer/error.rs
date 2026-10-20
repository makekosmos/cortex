/// SQLite failure detail that can cross the Store boundary without exposing
/// rusqlite types to callers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoreError {
    pub kind: StoreErrorKind,
    message: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoreErrorKind {
    DiskFull,
    Io,
    Corrupt,
    Other,
}

pub(crate) fn store_error_from_sqlite(error: rusqlite::Error) -> StoreError {
    let kind = match &error {
        rusqlite::Error::SqliteFailure(code, _) => match code.code {
            rusqlite::ErrorCode::DiskFull => StoreErrorKind::DiskFull,
            rusqlite::ErrorCode::SystemIoFailure => StoreErrorKind::Io,
            rusqlite::ErrorCode::DatabaseCorrupt | rusqlite::ErrorCode::NotADatabase => {
                StoreErrorKind::Corrupt
            }
            _ => StoreErrorKind::Other,
        },
        _ => StoreErrorKind::Other,
    };
    StoreError {
        kind,
        message: error.to_string(),
    }
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}
