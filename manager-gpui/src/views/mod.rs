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
    Connections,
    Keys,
    About,
    Updates,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StoreTab {
    Catalog,
    Installed,
}

pub const NAV_GROUPS: &[&[View]] = &[
    &[View::Packages, View::Data, View::Usage],
    &[View::Sync, View::Connections, View::Keys],
    &[View::Settings, View::Appearance, View::Updates, View::About],
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
            View::Connections => "Интеграции",
            View::Keys => "Ключи",
            View::About => "О приложении",
            View::Updates => "Обновления",
        }
    }

    pub fn icon(self) -> Icon {
        let path = match self {
            View::Data => "icons/database.svg",
            View::Usage => "icons/cpu.svg",
            View::Sync => "icons/device-monitor.svg",
            View::Packages => "icons/store.svg",
            View::Settings => "icons/settings.svg",
            View::Appearance => "icons/appearance.svg",
            View::Connections => "icons/connections.svg",
            View::Keys => "icons/key.svg",
            View::About => "icons/help-circle.svg",
            View::Updates => "icons/download.svg",
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
        View::Connections => connections::load(app),
        View::Keys => secrets::load(app),
        View::About => about::load(app),
        View::Updates => updates::load(app),
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
        View::Connections => connections::render(app, window, cx),
        View::Keys => secrets::render(app, window, cx),
        View::About => about::render(app, window, cx),
        View::Updates => updates::render(app, window, cx),
    }
}
