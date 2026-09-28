//! Descriptors for the packaged GPUI companion apps that the tray can launch
//! (Manager, Agenda, Memoria, Dictation). Each one ships next to the Engine
//! install as `resources/components/<dir_name>/<exe_name>`.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Component {
    Manager,
    Agenda,
    Memoria,
    Dictation,
}

impl Component {
    pub const ALL: [Component; 4] = [
        Component::Manager,
        Component::Agenda,
        Component::Memoria,
        Component::Dictation,
    ];

    /// Folder name under `resources/components/`.
    pub fn dir_name(self) -> &'static str {
        match self {
            Component::Manager => "manager",
            Component::Agenda => "agenda",
            Component::Memoria => "memoria",
            Component::Dictation => "dictation",
        }
    }

    /// Packaged executable file name inside its component folder.
    pub fn exe_name(self) -> &'static str {
        match self {
            Component::Manager => "Kosmos Manager.exe",
            Component::Agenda => "Kosmos Agenda.exe",
            Component::Memoria => "Kosmos Memoria.exe",
            Component::Dictation => "Kosmos Dictation.exe",
        }
    }

    /// Environment variable that overrides the resolved path for dev/local runs.
    pub fn env_override(self) -> &'static str {
        match self {
            Component::Manager => "KOSMOS_MANAGER_EXECUTABLE",
            Component::Agenda => "KOSMOS_AGENDA_EXECUTABLE",
            Component::Memoria => "KOSMOS_MEMORIA_EXECUTABLE",
            Component::Dictation => "KOSMOS_DICTATION_EXECUTABLE",
        }
    }

    /// Label shown for this component in the tray context menu.
    pub fn menu_label(self) -> &'static str {
        match self {
            Component::Manager => "Manager",
            Component::Agenda => "Agenda",
            Component::Memoria => "Memoria",
            Component::Dictation => "Диктовка",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Component;

    #[test]
    fn every_component_has_distinct_dir_and_exe_names() {
        let mut dirs: Vec<&str> = Component::ALL.iter().map(|c| c.dir_name()).collect();
        let mut exes: Vec<&str> = Component::ALL.iter().map(|c| c.exe_name()).collect();
        dirs.sort_unstable();
        dirs.dedup();
        exes.sort_unstable();
        exes.dedup();
        assert_eq!(dirs.len(), Component::ALL.len());
        assert_eq!(exes.len(), Component::ALL.len());
    }

    #[test]
    fn every_component_has_a_dedicated_env_override() {
        let mut vars: Vec<&str> = Component::ALL.iter().map(|c| c.env_override()).collect();
        vars.sort_unstable();
        vars.dedup();
        assert_eq!(vars.len(), Component::ALL.len());
    }
}
