//! Manager views grouped by purpose, with shared Imago icons.
use ::gpui::{prelude::*, *};
use gpui_component::Icon;

use crate::app::ManagerApp;

pub mod about;
pub mod appearance;
pub mod backups;
pub mod browser;
pub mod connections;
pub mod data;
pub mod dev;
pub mod engine_settings;
pub mod secrets;
pub mod settings;
pub mod store;
pub mod store_apps;
pub mod sync;
pub mod updates;
pub mod usage;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum View {
    Data,
    Usage,
    Sync,
    Packages,
    Settings,
    Appearance,
    Browser,
    Connections,
    Keys,
    About,
    Dev,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StoreTab {
    Catalog,
    Installed,
}

pub const NAV_GROUPS: &[&[View]] = &[
    &[View::About],
    &[View::Packages, View::Data, View::Usage],
    &[View::Sync, View::Connections, View::Keys],
    &[View::Settings, View::Appearance, View::Browser, View::Dev],
];

impl View {
    pub fn label(self) -> &'static str {
        match self {
            View::Data => "Данные",
            View::Usage => "Активность",
            View::Sync => "Девайсы",
            View::Packages => "Приложения",
            View::Settings => "Настройки",
            View::Appearance => "Внешний вид",
            View::Browser => "Браузер",
            View::Dev => "Разработчикам",
            View::Connections => "Интеграции",
            View::Keys => "Ключи",
            View::About => "О приложении",
        }
    }

    /// Stable persistence key for the last-opened page.
    pub fn key(self) -> &'static str {
        match self {
            View::Data => "data",
            View::Usage => "usage",
            View::Sync => "sync",
            View::Packages => "packages",
            View::Settings => "settings",
            View::Appearance => "appearance",
            View::Browser => "browser",
            View::Connections => "connections",
            View::Keys => "keys",
            View::About => "about",
            View::Dev => "dev",
        }
    }

    pub fn from_key(key: &str) -> Option<View> {
        Some(match key {
            "data" => View::Data,
            "usage" => View::Usage,
            "sync" => View::Sync,
            "packages" => View::Packages,
            "settings" => View::Settings,
            "appearance" => View::Appearance,
            "browser" => View::Browser,
            "connections" => View::Connections,
            "keys" => View::Keys,
            "about" => View::About,
            "updates" => View::About,
            "dev" => View::Dev,
            _ => return None,
        })
    }

    pub fn icon(self) -> Icon {
        let path = match self {
            View::Data => "icons/database.svg",
            View::Usage => "icons/cpu.svg",
            View::Sync => "icons/device-monitor.svg",
            View::Packages => "icons/store.svg",
            View::Settings => "icons/settings.svg",
            View::Appearance => "icons/appearance.svg",
            View::Browser => "icons/globe.svg",
            View::Dev => "icons/code.svg",
            View::Connections => "icons/connections.svg",
            View::Keys => "icons/key.svg",
            View::About => "icons/help-circle.svg",
        };
        Icon::default().path(path)
    }
}

/// Issue the ops a view needs when it becomes active or refreshes.
pub fn load(view: View, app: &mut ManagerApp) {
    match view {
        View::Data => data::load(app),
        View::Usage => usage::load(app),
        View::Sync => sync::load(app),
        View::Packages => store::load(app),
        View::Settings => settings::load(app),
        View::Appearance => appearance::load(app),
        View::Browser => browser::load(app),
        View::Dev => dev::load(app),
        View::Connections => connections::load(app),
        View::Keys => secrets::load(app),
        View::About => about::load(app),
    }
}

pub fn render(
    view: View,
    app: &mut ManagerApp,
    window: &mut Window,
    cx: &mut Context<ManagerApp>,
) -> AnyElement {
    match view {
        View::Data => data::render(app, window, cx),
        View::Usage => usage::render(app, window, cx),
        View::Sync => sync::render(app, window, cx),
        View::Packages => store::render(app, window, cx),
        View::Settings => settings::render(app, window, cx),
        View::Appearance => appearance::render(app, window, cx),
        View::Browser => browser::render(app, window, cx),
        View::Dev => dev::render(app, window, cx),
        View::Connections => connections::render(app, window, cx),
        View::Keys => secrets::render(app, window, cx),
        View::About => about::render(app, window, cx),
    }
}
