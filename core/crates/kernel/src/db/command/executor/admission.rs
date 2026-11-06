use super::super::CommandClass;
#[derive(Clone, Copy, Debug)]
pub struct AdmissionLimits {
    pub max_commands: usize,
    pub max_bytes: usize,
    pub max_command_bytes: usize,
    pub max_commands_per_source: usize,
    pub max_bytes_per_source: usize,
    pub max_sources: usize,
    pub reserved_interactive_sources: usize,
    pub reserved_sync_live_sources: usize,
    pub reserved_interactive_commands: usize,
    pub reserved_interactive_bytes: usize,
    pub reserved_sync_live_commands: usize,
    pub reserved_sync_live_bytes: usize,
}

impl AdmissionLimits {
    pub fn source_capacity(self, class: CommandClass) -> usize {
        match class {
            CommandClass::Interactive | CommandClass::Maintenance => self.max_sources,
            CommandClass::SyncLive => self.max_sources - self.reserved_interactive_sources,
            CommandClass::Backfill => {
                self.max_sources
                    - self.reserved_interactive_sources
                    - self.reserved_sync_live_sources
            }
        }
    }
}
