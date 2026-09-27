//! Pure tray menu model: which entries appear, in what order, and with
//! which Win32 command id — independent of `AppendMenuW`/`TrackPopupMenu`
//! so the ordering/labelling rules can be tested without a window.

use super::components::Component;

pub const MENU_OPEN: usize = 1;
pub const MENU_EXIT: usize = 2;
pub const MENU_MANAGER: usize = 3;
pub const MENU_AGENDA: usize = 4;
pub const MENU_MEMORIA: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    OpenCortex,
    OpenComponent(Component),
    Exit,
}

impl MenuAction {
    pub fn command_id(self) -> usize {
        match self {
            MenuAction::OpenCortex => MENU_OPEN,
            MenuAction::OpenComponent(Component::Manager) => MENU_MANAGER,
            MenuAction::OpenComponent(Component::Agenda) => MENU_AGENDA,
            MenuAction::OpenComponent(Component::Memoria) => MENU_MEMORIA,
            MenuAction::Exit => MENU_EXIT,
        }
    }

    pub fn from_command_id(id: usize) -> Option<MenuAction> {
        match id {
            MENU_OPEN => Some(MenuAction::OpenCortex),
            MENU_MANAGER => Some(MenuAction::OpenComponent(Component::Manager)),
            MENU_AGENDA => Some(MenuAction::OpenComponent(Component::Agenda)),
            MENU_MEMORIA => Some(MenuAction::OpenComponent(Component::Memoria)),
            MENU_EXIT => Some(MenuAction::Exit),
            _ => None,
        }
    }

    /// `Открыть`/`Выход` keep their historic labels (the Cortex-open entry
    /// predates this menu and the Electron tray it replaced used the same
    /// wording); GPUI components are labelled by name, matching how the
    /// command palette lists them (`Открыть Agenda (GPUI)` etc. shortened
    /// to fit a context menu).
    pub fn label(self) -> &'static str {
        match self {
            MenuAction::OpenCortex => "Открыть",
            MenuAction::OpenComponent(component) => component.menu_label(),
            MenuAction::Exit => "Выход",
        }
    }
}

/// Which launch targets were actually found on disk — see
/// `resolve::resolve_cortex_executable` / `resolve_component_executable`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MenuPresence {
    pub cortex: bool,
    pub manager: bool,
    pub agenda: bool,
    pub memoria: bool,
}

impl MenuPresence {
    fn has(self, component: Component) -> bool {
        match component {
            Component::Manager => self.manager,
            Component::Agenda => self.agenda,
            Component::Memoria => self.memoria,
        }
    }
}

pub struct MenuEntry {
    pub action: MenuAction,
    pub separator_before: bool,
}

/// Builds the ordered tray menu for the given presence of launch targets.
/// `Выход` is always present; every open-action appears only when its
/// executable was actually found, extending the historic
/// `menu_commands(cortex_present)` behaviour to the GPUI components that
/// reach parity with the (removed) Electron tray.
pub fn build_menu(presence: MenuPresence) -> Vec<MenuEntry> {
    let mut entries = Vec::new();
    if presence.cortex {
        entries.push(MenuAction::OpenCortex);
    }
    for component in Component::ALL {
        if presence.has(component) {
            entries.push(MenuAction::OpenComponent(component));
        }
    }
    let separator_before_exit = !entries.is_empty();
    entries
        .into_iter()
        .map(|action| MenuEntry {
            action,
            separator_before: false,
        })
        .chain(std::iter::once(MenuEntry {
            action: MenuAction::Exit,
            separator_before: separator_before_exit,
        }))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_present_still_offers_exit_only() {
        let entries = build_menu(MenuPresence::default());
        let ids: Vec<usize> = entries.iter().map(|e| e.action.command_id()).collect();
        assert_eq!(ids, vec![MENU_EXIT]);
        assert!(!entries[0].separator_before);
    }

    #[test]
    fn cortex_presence_controls_the_exact_tray_menu() {
        let presence = MenuPresence {
            cortex: true,
            ..Default::default()
        };
        let ids: Vec<usize> = build_menu(presence)
            .iter()
            .map(|e| e.action.command_id())
            .collect();
        assert_eq!(ids, vec![MENU_OPEN, MENU_EXIT]);

        let ids: Vec<usize> = build_menu(MenuPresence::default())
            .iter()
            .map(|e| e.action.command_id())
            .collect();
        assert_eq!(ids, vec![MENU_EXIT]);
    }

    #[test]
    fn every_present_component_appears_in_a_fixed_order() {
        let presence = MenuPresence {
            cortex: true,
            manager: true,
            agenda: true,
            memoria: true,
        };
        let ids: Vec<usize> = build_menu(presence)
            .iter()
            .map(|e| e.action.command_id())
            .collect();
        assert_eq!(
            ids,
            vec![
                MENU_OPEN,
                MENU_MANAGER,
                MENU_AGENDA,
                MENU_MEMORIA,
                MENU_EXIT
            ]
        );
    }

    #[test]
    fn a_missing_component_is_simply_skipped() {
        let presence = MenuPresence {
            cortex: false,
            manager: true,
            agenda: false,
            memoria: true,
        };
        let ids: Vec<usize> = build_menu(presence)
            .iter()
            .map(|e| e.action.command_id())
            .collect();
        assert_eq!(ids, vec![MENU_MANAGER, MENU_MEMORIA, MENU_EXIT]);
    }

    #[test]
    fn separator_only_precedes_exit_when_something_else_is_offered() {
        assert!(!build_menu(MenuPresence::default())[0].separator_before);
        let presence = MenuPresence {
            manager: true,
            ..Default::default()
        };
        let entries = build_menu(presence);
        assert!(!entries[0].separator_before);
        assert!(entries[1].separator_before);
    }

    #[test]
    fn command_id_round_trips_through_from_command_id() {
        for action in [
            MenuAction::OpenCortex,
            MenuAction::OpenComponent(Component::Manager),
            MenuAction::OpenComponent(Component::Agenda),
            MenuAction::OpenComponent(Component::Memoria),
            MenuAction::Exit,
        ] {
            assert_eq!(
                MenuAction::from_command_id(action.command_id()),
                Some(action)
            );
        }
        assert_eq!(MenuAction::from_command_id(0), None);
    }

    #[test]
    fn open_and_exit_keep_their_historic_russian_labels() {
        assert_eq!(MenuAction::OpenCortex.label(), "Открыть");
        assert_eq!(MenuAction::Exit.label(), "Выход");
    }
}
