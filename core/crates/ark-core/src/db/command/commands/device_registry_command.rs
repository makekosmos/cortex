use ark_domain::device_registry::RegistryCommand;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct AppendLocalRegistryEventCommand {
    actor_device_id: String,
    command: RegistryCommand,
}

impl AppendLocalRegistryEventCommand {
    pub(crate) fn new(actor_device_id: String, command: RegistryCommand) -> Self {
        Self {
            actor_device_id,
            command,
        }
    }

    pub(crate) fn into_parts(self) -> (String, RegistryCommand) {
        (self.actor_device_id, self.command)
    }
}
