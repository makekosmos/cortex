//! Pure tray menu model: which entries appear, in what order, and with
//! which Win32 command id — independent of `AppendMenuW`/`TrackPopupMenu`
//! so the ordering/labelling rules can be tested without a window.

use super::components::Component;
use std::collections::BTreeSet;

pub const MENU_OPEN: usize = 1;
pub const MENU_EXIT: usize = 2;
/// Component items get `MENU_COMPONENT_BASE + position in Component::all()`
/// — deterministic because the descriptor table order is.
const MENU_COMPONENT_BASE: usize = 10;

fn component_command_id(component: Component) -> usize {
    MENU_COMPONENT_BASE
        + Component::all()
            .position(|candidate| candidate == component)
            .unwrap_or(0)
}

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
            MenuAction::OpenComponent(component) => component_command_id(component),
            MenuAction::Exit => MENU_EXIT,
        }
    }

    pub fn from_command_id(id: usize) -> Option<MenuAction> {
        match id {
            MENU_OPEN => Some(MenuAction::OpenManager),
            MENU_EXIT => Some(MenuAction::Exit),
            _ => Component::all()
                .find(|component| component_command_id(*component) == id)
                .map(MenuAction::OpenComponent),
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

/// Which launch targets were actually found on disk — command keys of the
/// resolved components (see `resolve_component_executable`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MenuPresence(pub BTreeSet<&'static str>);

impl MenuPresence {
    fn has(&self, component: Component) -> bool {
        self.0.contains(component.command_key())
    }
}

pub struct MenuEntry {
    pub action: MenuAction,
    pub separator_before: bool,
}

/// Builds the ordered tray menu for the given presence of launch targets.
/// `Выход` is always present; every open-action appears only when its
/// executable was actually found.
pub fn build_menu(presence: &MenuPresence) -> Vec<MenuEntry> {
    let mut entries = Vec::new();
    if presence.has(Component::Manager) {
        // The historic "Открыть" entry opens the Manager GPUI.
        entries.push(MenuAction::OpenManager);
    }
    for component in Component::all() {
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

    fn presence(components: &[&'static str]) -> MenuPresence {
        MenuPresence(components.iter().copied().collect())
    }

    fn command_ids(entries: &[MenuEntry]) -> Vec<usize> {
        entries.iter().map(|e| e.action.command_id()).collect()
    }

    #[test]
    fn nothing_present_still_offers_exit_only() {
        let entries = build_menu(&MenuPresence::default());
        assert_eq!(command_ids(&entries), vec![MENU_EXIT]);
        assert!(!entries[0].separator_before);
    }

    #[test]
    fn manager_presence_controls_the_open_entry() {
        let ids = command_ids(&build_menu(&presence(&["manager"])));
        assert_eq!(ids, vec![MENU_OPEN, MENU_EXIT]);
        let ids = command_ids(&build_menu(&MenuPresence::default()));
        assert_eq!(ids, vec![MENU_EXIT]);
    }

    #[test]
    fn every_present_component_appears_in_descriptor_order() {
        let all: Vec<&'static str> = Component::all().map(|c| c.command_key()).collect();
        let ids = command_ids(&build_menu(&presence(&all)));
        // Manager first (as "Открыть"), then apps in table order, then Exit.
        let expected: Vec<usize> = Component::all()
            .skip(1)
            .map(|c| MenuAction::OpenComponent(c).command_id())
            .collect();
        let mut expected_full = vec![MENU_OPEN];
        expected_full.extend(expected);
        expected_full.push(MENU_EXIT);
        assert_eq!(ids, expected_full);
    }

    #[test]
    fn a_missing_component_is_simply_skipped() {
        let ids = command_ids(&build_menu(&presence(&["manager", "com.kosmos.memoria"])));
        assert_eq!(ids.len(), 3);
        assert_eq!(ids[0], MENU_OPEN);
        assert_eq!(ids[2], MENU_EXIT);
    }

    #[test]
    fn separator_only_precedes_exit_when_something_else_is_offered() {
        assert!(!build_menu(&MenuPresence::default())[0].separator_before);
        let entries = build_menu(&presence(&["manager"]));
        assert!(!entries[0].separator_before);
        let last = entries.last().expect("exit entry is always present");
        assert!(matches!(last.action, MenuAction::Exit));
        assert!(last.separator_before);
    }

    #[test]
    fn command_id_round_trips_through_from_command_id() {
        let actions: Vec<MenuAction> = std::iter::once(MenuAction::OpenManager)
            .chain(Component::all().map(MenuAction::OpenComponent))
            .chain(std::iter::once(MenuAction::Exit))
            .collect();
        for action in actions {
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
