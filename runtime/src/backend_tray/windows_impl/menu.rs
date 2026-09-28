//! Pure tray menu model: which entries appear, in what order, and with
//! which Win32 command id — independent of `AppendMenuW`/`TrackPopupMenu`
//! so the ordering/labelling rules can be tested without a window.

use super::components::Component;

pub const MENU_OPEN: usize = 1;
pub const MENU_EXIT: usize = 2;
pub const MENU_MANAGER: usize = 3;
pub const MENU_AGENDA: usize = 4;
pub const MENU_MEMORIA: usize = 5;
pub const MENU_DICTATION: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    /// "Открыть" launches the Manager GPUI.
    OpenManager,
    OpenComponent(Component),
    Exit,
}

impl MenuAction {
    pub fn command_id(self) -> usize {
        match self {
            MenuAction::OpenManager => MENU_OPEN,
            MenuAction::OpenComponent(Component::Manager) => MENU_MANAGER,
            MenuAction::OpenComponent(Component::Agenda) => MENU_AGENDA,
            MenuAction::OpenComponent(Component::Memoria) => MENU_MEMORIA,
            MenuAction::OpenComponent(Component::Dictation) => MENU_DICTATION,
            MenuAction::Exit => MENU_EXIT,
        }
    }

    pub fn from_command_id(id: usize) -> Option<MenuAction> {
        match id {
            MENU_OPEN => Some(MenuAction::OpenManager),
            MENU_MANAGER => Some(MenuAction::OpenComponent(Component::Manager)),
            MENU_AGENDA => Some(MenuAction::OpenComponent(Component::Agenda)),
            MENU_MEMORIA => Some(MenuAction::OpenComponent(Component::Memoria)),
            MENU_DICTATION => Some(MenuAction::OpenComponent(Component::Dictation)),
            MENU_EXIT => Some(MenuAction::Exit),
            _ => None,
        }
    }

    /// `Открыть`/`Выход` keep their historic labels; GPUI components are
    /// labelled by name, matching how the command palette lists them.
    pub fn label(self) -> &'static str {
        match self {
            MenuAction::OpenManager => "Открыть",
            MenuAction::OpenComponent(component) => component.menu_label(),
            MenuAction::Exit => "Выход",
        }
    }
}

/// Which launch targets were actually found on disk — see `resolve_component_executable`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MenuPresence {
    pub manager: bool,
    pub agenda: bool,
    pub memoria: bool,
    pub dictation: bool,
}

impl MenuPresence {
    fn has(self, component: Component) -> bool {
        match component {
            Component::Manager => self.manager,
            Component::Agenda => self.agenda,
            Component::Memoria => self.memoria,
            Component::Dictation => self.dictation,
        }
    }
}

pub struct MenuEntry {
    pub action: MenuAction,
    pub separator_before: bool,
}

/// Builds the ordered tray menu for the given presence of launch targets.
/// `Выход` is always present; every open-action appears only when its
/// executable was actually found.
pub fn build_menu(presence: MenuPresence) -> Vec<MenuEntry> {
    let mut entries = Vec::new();
    if presence.manager {
        // The historic "Открыть" entry opens the Manager GPUI.
        entries.push(MenuAction::OpenManager);
    }
    for component in Component::ALL {
        // Manager is already reachable through the historic "Открыть" entry.
        if component != Component::Manager && presence.has(component) {
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
    fn manager_presence_controls_the_open_entry() {
        let presence = MenuPresence {
            manager: true,
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
            manager: true,
            agenda: true,
            memoria: true,
            dictation: true,
        };
        let ids: Vec<usize> = build_menu(presence)
            .iter()
            .map(|e| e.action.command_id())
            .collect();
        assert_eq!(
            ids,
            vec![
                MENU_OPEN,
                MENU_AGENDA,
                MENU_MEMORIA,
                MENU_DICTATION,
                MENU_EXIT
            ]
        );
    }

    #[test]
    fn a_missing_component_is_simply_skipped() {
        let presence = MenuPresence {
            manager: true,
            agenda: false,
            memoria: true,
            dictation: false,
        };
        let ids: Vec<usize> = build_menu(presence)
            .iter()
            .map(|e| e.action.command_id())
            .collect();
        assert_eq!(ids, vec![MENU_OPEN, MENU_MEMORIA, MENU_EXIT]);
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
        let last = entries.last().expect("exit entry is always present");
        assert!(matches!(last.action, MenuAction::Exit));
        assert!(last.separator_before);
    }

    #[test]
    fn command_id_round_trips_through_from_command_id() {
        for action in [
            MenuAction::OpenManager,
            MenuAction::OpenComponent(Component::Manager),
            MenuAction::OpenComponent(Component::Agenda),
            MenuAction::OpenComponent(Component::Memoria),
            MenuAction::OpenComponent(Component::Dictation),
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
        assert_eq!(MenuAction::OpenManager.label(), "Открыть");
        assert_eq!(MenuAction::Exit.label(), "Выход");
    }
}
