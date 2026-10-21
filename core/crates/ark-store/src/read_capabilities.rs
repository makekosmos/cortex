mod canonical;
mod device_registry;
mod integration_replication;
mod objects;
mod sync;
mod system;
mod task_merge;
mod type_registry;
mod usage;

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

pub struct UsageReadCapabilities<'snapshot> {
    connection: &'snapshot Connection,
}

pub struct TypeRegistryReadCapabilities<'snapshot> {
    connection: &'snapshot Connection,
}

pub struct DeviceRegistryReadCapabilities<'snapshot> {
    connection: &'snapshot Connection,
}

pub struct SystemReadCapabilities<'snapshot> {
    connection: &'snapshot Connection,
}

pub struct TaskMergeReadCapabilities<'snapshot> {
    connection: &'snapshot Connection,
}

pub struct SyncReadCapabilities<'snapshot> {
    connection: &'snapshot Connection,
}

pub struct IntegrationReplicationReadCapabilities<'snapshot> {
    connection: &'snapshot Connection,
}

impl<'snapshot> ReadCapabilities<'snapshot> {
    pub(crate) fn new(connection: &'snapshot Connection) -> Self {
        Self { connection }
    }

    #[cfg(feature = "test-support")]
    pub fn test_support(&self) -> crate::test_support::ActorTestReads<'_> {
        crate::test_support::ActorTestReads::new(self.connection)
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

    pub fn usage(&self) -> UsageReadCapabilities<'_> {
        UsageReadCapabilities {
            connection: self.connection,
        }
    }

    pub fn type_registry(&self) -> TypeRegistryReadCapabilities<'_> {
        TypeRegistryReadCapabilities {
            connection: self.connection,
        }
    }

    pub fn device_registry(&self) -> DeviceRegistryReadCapabilities<'_> {
        DeviceRegistryReadCapabilities {
            connection: self.connection,
        }
    }

    pub fn system(&self) -> SystemReadCapabilities<'_> {
        SystemReadCapabilities {
            connection: self.connection,
        }
    }

    pub fn sync(&self) -> SyncReadCapabilities<'_> {
        SyncReadCapabilities {
            connection: self.connection,
        }
    }

    pub fn integration_replication(&self) -> IntegrationReplicationReadCapabilities<'_> {
        IntegrationReplicationReadCapabilities {
            connection: self.connection,
        }
    }

    pub fn task_merge(&self) -> TaskMergeReadCapabilities<'_> {
        TaskMergeReadCapabilities {
            connection: self.connection,
        }
    }
}
