//! Pure tray menu model: which entries appear, in what order, and with
//! which Win32 command id — independent of `AppendMenuW`/`TrackPopupMenu`
//! so the ordering/labelling rules can be tested without a window.
//!
//! The contract is the macOS menu bar helper's (`native/macos/engine-menu-bar.swift`):
//! one entry that opens the interface, one that quits the Engine, nothing
//! else. Opening an app is the Manager's business, not the tray's.

pub const MENU_OPEN: usize = 1;
pub const MENU_EXIT: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    /// "Открыть интерфейс" launches the Manager GPUI.
    OpenManager,
    Exit,
}

impl MenuAction {
    pub fn command_id(self) -> usize {
        match self {
            MenuAction::OpenManager => MENU_OPEN,
            MenuAction::Exit => MENU_EXIT,
        }
    }

    pub fn from_command_id(id: usize) -> Option<MenuAction> {
        match id {
            MENU_OPEN => Some(MenuAction::OpenManager),
            MENU_EXIT => Some(MenuAction::Exit),
            _ => None,
        }
    }

    /// The labels the macOS helper uses, so the menu reads the same on both.
    pub fn label(self) -> &'static str {
        match self {
            MenuAction::OpenManager => "Открыть интерфейс",
            MenuAction::Exit => "Выход",
        }
    }
}

pub struct MenuEntry {
    pub action: MenuAction,
    pub separator_before: bool,
}

/// Builds the tray menu: open, a separator, quit — the macOS helper's menu.
pub fn build_menu() -> Vec<MenuEntry> {
    vec![
        MenuEntry {
            action: MenuAction::OpenManager,
            separator_before: false,
        },
        MenuEntry {
            action: MenuAction::Exit,
            separator_before: true,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command_ids(entries: &[MenuEntry]) -> Vec<usize> {
        entries.iter().map(|e| e.action.command_id()).collect()
    }

    #[test]
    fn the_menu_is_open_then_quit_only() {
        let entries = build_menu();
        assert_eq!(command_ids(&entries), vec![MENU_OPEN, MENU_EXIT]);
        assert!(matches!(entries[0].action, MenuAction::OpenManager));
        assert!(matches!(entries[1].action, MenuAction::Exit));
    }

    #[test]
    fn separator_only_precedes_exit() {
        let entries = build_menu();
        assert!(!entries[0].separator_before);
        assert!(entries[1].separator_before);
    }

    #[test]
    fn command_id_round_trips_through_from_command_id() {
        for action in [MenuAction::OpenManager, MenuAction::Exit] {
            assert_eq!(
                MenuAction::from_command_id(action.command_id()),
                Some(action)
            );
        }
        assert_eq!(MenuAction::from_command_id(0), None);
        assert_eq!(MenuAction::from_command_id(3), None);
    }

    #[test]
    fn labels_match_the_macos_helper() {
        // native/macos/engine-menu-bar.swift: "Открыть интерфейс", "Выход".
        assert_eq!(MenuAction::OpenManager.label(), "Открыть интерфейс");
        assert_eq!(MenuAction::Exit.label(), "Выход");
    }
}
