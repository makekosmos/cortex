//! Which launch targets the tray can offer. Only Manager is still bundled
//! next to the Engine (`resources/components/manager`); Agenda, Memoria and
//! Dictation are store-installed native apps — their ids, executable names
//! and env overrides come straight from `native_apps::NATIVE_APPS`, so this
//! file holds no second copy of that table.

use engine::native_apps::NativeAppDescriptor;

#[derive(Debug, Clone, Copy)]
pub enum Component {
    Manager,
    App(&'static NativeAppDescriptor),
}

impl Component {
    /// Manager first, then every store app in descriptor order.
    pub fn all() -> impl Iterator<Item = Component> {
        std::iter::once(Component::Manager)
            .chain(engine::native_apps::NATIVE_APPS.iter().map(Component::App))
    }

    /// The store descriptor for apps; `None` for the bundled Manager.
    pub fn app_descriptor(self) -> Option<&'static NativeAppDescriptor> {
        match self {
            Component::Manager => None,
            Component::App(desc) => Some(desc),
        }
    }

    /// Folder name under `resources/components/` — bundled Manager only.
    pub fn dir_name(self) -> Option<&'static str> {
        match self {
            Component::Manager => Some("manager"),
            Component::App(_) => None,
        }
    }

    /// Packaged executable file name inside the component folder.
    pub fn exe_name(self) -> Option<&'static str> {
        match self {
            Component::Manager => Some("Mundus Manager.exe"),
            Component::App(_) => None,
        }
    }

    /// `brand::env` suffix for the dev override (`MUNDUS_*_EXECUTABLE`, with
    /// legacy-name fallback) — the same source the service uses.
    pub fn env_override(self) -> &'static str {
        match self {
            Component::Manager => "MANAGER_EXECUTABLE",
            Component::App(desc) => desc.env_override,
        }
    }

    /// Label shown for this component in the tray context menu.
    pub fn menu_label(self) -> &'static str {
        match self {
            Component::Manager => "Manager",
            Component::App(desc) => match desc.id {
                "com.kosmos.dictation" => "Диктовка",
                _ => desc.name,
            },
        }
    }
}

impl PartialEq for Component {
    fn eq(&self, other: &Self) -> bool {
        self.command_key() == other.command_key()
    }
}
impl Eq for Component {}

impl Component {
    /// Identity for menu bookkeeping — the descriptor id for apps.
    pub fn command_key(self) -> &'static str {
        match self {
            Component::Manager => "manager",
            Component::App(desc) => desc.id,
        }
    }
}
