//! Manager views grouped by purpose, with shared Imago icons.
use ::gpui::{prelude::*, *};
use gpui_component::Icon;

use crate::app::ManagerApp;

pub mod about;
pub mod browser;
pub mod connections;
pub mod data;
pub mod dev;
pub mod dictation;
mod dictation_cards;
mod dictation_local;
pub mod engine_settings;
pub mod secrets;
pub mod settings;
pub mod store;
pub mod sync;
pub mod updates;
pub mod usage;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum View {
    Data,
    Usage,
    Sync,
    Packages,
    Engine,
    Settings,
    Connections,
    About,
    Updates,
    Secrets,
    Browser,
    Dev,
    Dictation,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StoreTab {
    Catalog,
    Installed,
}

pub const NAV_GROUPS: &[&[View]] = &[
    &[View::Data, View::Usage, View::Packages, View::Browser],
    &[
        View::Sync,
        View::Connections,
        View::Dictation,
        View::Secrets,
    ],
    &[View::Settings, View::Engine, View::Dev],
    &[View::Updates, View::About],
];

impl View {
    pub fn label(self) -> &'static str {
        match self {
            View::Data => "Данные",
            View::Usage => "Затреканное время",
            View::Sync => "Синхронизация",
            View::Packages => "Маркетплейс",
            View::Engine => "Движок",
            View::Settings => "Настройки",
            View::Connections => "Интеграции",
            View::About => "О приложении",
            View::Updates => "Обновления",
            View::Secrets => "Ключи",
            View::Browser => "Браузер",
            View::Dev => "Разработка",
            View::Dictation => "Диктовка",
        }
    }

    pub fn icon(self) -> Icon {
        let path = match self {
            View::Data => "icons/database.svg",
            View::Usage => "icons/cpu.svg",
            View::Sync => "icons/sync.svg",
            View::Packages => "icons/store.svg",
            View::Engine => "icons/cpu.svg",
            View::Settings => "icons/settings.svg",
            View::Connections => "icons/connections.svg",
            View::About => "icons/help-circle.svg",
            View::Updates => "icons/download.svg",
            View::Secrets => "icons/key.svg",
            View::Browser => "icons/globe.svg",
            View::Dev => "icons/code.svg",
            View::Dictation => "icons/mic.svg",
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
        View::Engine => engine_settings::load(app),
        View::Settings => settings::load(app),
        View::Connections => connections::load(app),
        View::About => about::load(app),
        View::Updates => updates::load(app),
        View::Secrets => secrets::load(app),
        View::Browser => browser::load(app),
        View::Dev => dev::load(app),
        View::Dictation => dictation::load(app),
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
        View::Engine => engine_settings::render(app, window, cx),
        View::Settings => settings::render(app, window, cx),
        View::Connections => connections::render(app, window, cx),
        View::About => about::render(app, window, cx),
        View::Updates => updates::render(app, window, cx),
        View::Secrets => secrets::render(app, window, cx),
        View::Browser => browser::render(app, window, cx),
        View::Dev => dev::render(app, window, cx),
        View::Dictation => dictation::render(app, window, cx),
    }
}
