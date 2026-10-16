use super::super::{CommandClass, CommandSource};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug)]
pub struct AdmissionLimits {
    pub max_commands: usize,
    pub max_bytes: usize,
    pub max_command_bytes: usize,
    pub max_commands_per_source: usize,
    pub max_bytes_per_source: usize,
    pub max_sources: usize,
    pub reserved_interactive_commands: usize,
    pub reserved_interactive_bytes: usize,
}

#[derive(Clone)]
pub struct AdmissionBudget {
    limits: AdmissionLimits,
    usage: Arc<Mutex<Usage>>,
}

#[derive(Default)]
struct Usage {
    commands: usize,
    bytes: usize,
    by_source: HashMap<CommandSource, SourceUsage>,
}

#[derive(Default)]
struct SourceUsage {
    commands: usize,
    bytes: usize,
}

pub struct AdmissionPermit {
    usage: Arc<Mutex<Usage>>,
    source: CommandSource,
    bytes: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdmissionError {
    TooLarge,
    Saturated,
}

impl AdmissionBudget {
    pub fn new(limits: AdmissionLimits) -> Self {
        assert!(limits.max_commands > limits.reserved_interactive_commands);
        assert!(limits.max_bytes > limits.reserved_interactive_bytes);
        assert!(limits.max_command_bytes > 0);
        assert!(limits.max_sources > 0);
        Self {
            limits,
            usage: Arc::new(Mutex::new(Usage::default())),
        }
    }

    pub fn try_acquire(
        &self,
        class: CommandClass,
        source: &CommandSource,
        bytes: usize,
    ) -> Result<AdmissionPermit, AdmissionError> {
        if bytes == 0 || bytes > self.limits.max_command_bytes {
            return Err(AdmissionError::TooLarge);
        }

        let mut usage = self.usage.lock().unwrap_or_else(|error| error.into_inner());
        let source_usage = usage.by_source.get(source);
        let next_source_commands = source_usage.map_or(1, |value| value.commands + 1);
        let next_source_bytes = source_usage.map_or(bytes, |value| value.bytes + bytes);
        if (source_usage.is_none() && usage.by_source.len() >= self.limits.max_sources)
            || next_source_commands > self.limits.max_commands_per_source
            || next_source_bytes > self.limits.max_bytes_per_source
            || usage.commands + 1 > self.limits.max_commands
            || usage.bytes + bytes > self.limits.max_bytes
        {
            return Err(AdmissionError::Saturated);
        }

        if class != CommandClass::Interactive
            && (usage.commands + 1
                > self.limits.max_commands - self.limits.reserved_interactive_commands
                || usage.bytes + bytes
                    > self.limits.max_bytes - self.limits.reserved_interactive_bytes)
        {
            return Err(AdmissionError::Saturated);
        }

        usage.commands += 1;
        usage.bytes += bytes;
        let source_usage = usage.by_source.entry(source.clone()).or_default();
        source_usage.commands += 1;
        source_usage.bytes += bytes;

        Ok(AdmissionPermit {
            usage: Arc::clone(&self.usage),
            source: source.clone(),
            bytes,
        })
    }
}

impl Drop for AdmissionPermit {
    fn drop(&mut self) {
        let mut usage = self.usage.lock().unwrap_or_else(|error| error.into_inner());
        usage.commands -= 1;
        usage.bytes -= self.bytes;
        let source_usage = usage
            .by_source
            .get_mut(&self.source)
            .expect("admitted source has usage");
        source_usage.commands -= 1;
        source_usage.bytes -= self.bytes;
        if source_usage.commands == 0 {
            usage.by_source.remove(&self.source);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits() -> AdmissionLimits {
        AdmissionLimits {
            max_commands: 4,
            max_bytes: 100,
            max_command_bytes: 80,
            max_commands_per_source: 2,
            max_bytes_per_source: 80,
            max_sources: 2,
            reserved_interactive_commands: 1,
            reserved_interactive_bytes: 20,
        }
    }

    fn engine() -> CommandSource {
        CommandSource::Engine
    }

    fn peer(id: &str) -> CommandSource {
        CommandSource::AuthenticatedPeer(id.to_owned())
    }

    #[test]
    fn releases_capacity_when_work_finishes() {
        let budget = AdmissionBudget::new(limits());
        let first = budget
            .try_acquire(CommandClass::Interactive, &engine(), 60)
            .unwrap();
        assert!(matches!(
            budget.try_acquire(CommandClass::Interactive, &engine(), 40),
            Err(AdmissionError::Saturated)
        ));
        drop(first);
        assert!(budget
            .try_acquire(CommandClass::Interactive, &engine(), 60)
            .is_ok());
    }

    #[test]
    fn reserves_global_capacity_for_interactive_commands() {
        let budget = AdmissionBudget::new(limits());
        let sync = budget
            .try_acquire(CommandClass::SyncLive, &peer("peer-a"), 80)
            .unwrap();
        assert!(matches!(
            budget.try_acquire(CommandClass::Backfill, &peer("peer-b"), 1),
            Err(AdmissionError::Saturated)
        ));
        assert!(budget
            .try_acquire(CommandClass::Interactive, &engine(), 20)
            .is_ok());
        drop(sync);
    }

    #[test]
    fn bounds_per_source_commands_bytes_and_source_metadata() {
        let budget = AdmissionBudget::new(limits());
        let first = budget
            .try_acquire(CommandClass::SyncLive, &peer("peer-a"), 30)
            .unwrap();
        let second = budget
            .try_acquire(CommandClass::SyncLive, &peer("peer-a"), 30)
            .unwrap();
        assert!(matches!(
            budget.try_acquire(CommandClass::SyncLive, &peer("peer-a"), 21),
            Err(AdmissionError::Saturated)
        ));
        let other = budget
            .try_acquire(CommandClass::SyncLive, &peer("peer-b"), 1)
            .unwrap();
        assert!(matches!(
            budget.try_acquire(CommandClass::SyncLive, &CommandSource::System, 1),
            Err(AdmissionError::Saturated)
        ));
        drop((first, second, other));
        assert!(budget
            .try_acquire(CommandClass::SyncLive, &CommandSource::System, 1)
            .is_ok());
    }

    #[test]
    fn rejects_oversized_commands_before_reserving_capacity() {
        let budget = AdmissionBudget::new(limits());
        assert!(matches!(
            budget.try_acquire(CommandClass::Interactive, &engine(), 81),
            Err(AdmissionError::TooLarge)
        ));
        assert!(budget
            .try_acquire(CommandClass::Interactive, &engine(), 80)
            .is_ok());
    }
}
