mod canonical;
mod objects;

use rusqlite::Connection;

pub struct ReadCapabilities<'snapshot> {
    connection: &'snapshot Connection,
}

pub struct ObjectReadCapabilities<'snapshot> {
    connection: &'snapshot Connection,
}

pub struct CanonicalReadCapabilities<'snapshot> {
    connection: &'snapshot Connection,
}

impl<'snapshot> ReadCapabilities<'snapshot> {
    pub(crate) fn new(connection: &'snapshot Connection) -> Self {
        Self { connection }
    }

    pub fn objects(&self) -> ObjectReadCapabilities<'_> {
        ObjectReadCapabilities {
            connection: self.connection,
        }
    }

    pub fn canonical(&self) -> CanonicalReadCapabilities<'_> {
        CanonicalReadCapabilities {
            connection: self.connection,
        }
    }
}
