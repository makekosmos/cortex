//! Descriptors for the packaged GPUI companion apps that the Electron shell
//! used to launch from its own tray/command palette (Manager, Agenda,
//! Memoria). Each one ships next to the Cortex shell install as
//! `resources/components/<dir_name>/<exe_name>` — see
//! `desktop/electron/manager-navigation.ts`, `agenda-navigation.ts` and
//! `memoria-navigation.ts` for the Electron-side resolution this mirrors.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Component {
    Manager,
    Agenda,
    Memoria,
}

impl Component {
    pub const ALL: [Component; 3] = [Component::Manager, Component::Agenda, Component::Memoria];

    /// Folder name under `resources/components/`.
    pub fn dir_name(self) -> &'static str {
        match self {
            Component::Manager => "manager",
            Component::Agenda => "agenda",
            Component::Memoria => "memoria",
        }
    }

    /// Packaged executable file name inside its component folder.
    pub fn exe_name(self) -> &'static str {
        match self {
            Component::Manager => "Kosmos Manager.exe",
            Component::Agenda => "Kosmos Agenda.exe",
            Component::Memoria => "Kosmos Memoria.exe",
        }
    }

    /// Environment variable that overrides the resolved path, mirroring
    /// `KOSMOS_MANAGER_EXECUTABLE` / `KOSMOS_AGENDA_EXECUTABLE` /
    /// `KOSMOS_MEMORIA_EXECUTABLE` used by the Electron navigation modules
    /// for dev/local runs.
    pub fn env_override(self) -> &'static str {
        match self {
            Component::Manager => "KOSMOS_MANAGER_EXECUTABLE",
            Component::Agenda => "KOSMOS_AGENDA_EXECUTABLE",
            Component::Memoria => "KOSMOS_MEMORIA_EXECUTABLE",
        }
    }

    /// Label shown for this component in the tray context menu.
    pub fn menu_label(self) -> &'static str {
        match self {
            Component::Manager => "Manager",
            Component::Agenda => "Agenda",
            Component::Memoria => "Memoria",
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
