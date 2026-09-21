//! View registry mirroring ManagerRoot.vue's nav table: same order, same
//! Russian labels/hints, Lucide icons matching the Phosphor set.
use ::gpui::assets::IconName;
use ::gpui::{prelude::*, *};

use crate::app::ManagerApp;

pub mod about;
pub mod browser;
pub mod connections;
pub mod data;
pub mod dev;
pub mod engine_settings;
pub mod secrets;
pub mod settings;
pub mod store;
pub mod sync;
pub mod updates;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum View {
    Data,
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
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StoreTab {
    Catalog,
    Installed,
}

pub const ALL: &[View] = &[
    View::Data,
    View::Sync,
    View::Packages,
    View::Engine,
    View::Settings,
    View::Connections,
    View::About,
    View::Updates,
    View::Secrets,
    View::Browser,
    View::Dev,
];

impl View {
    pub fn label(self) -> &'static str {
        match self {
            View::Data => "Данные",
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
        }
    }

    pub fn icon(self) -> IconName {
        match self {
            View::Data => IconName::Database,
            View::Sync => IconName::RefreshCcw,
            View::Packages => IconName::Store,
            View::Engine => IconName::Cpu,
            View::Settings => IconName::Settings,
            View::Connections => IconName::PlugZap,
            View::About => IconName::Info,
            View::Updates => IconName::Package,
            View::Secrets => IconName::Key,
            View::Browser => IconName::Globe,
            View::Dev => IconName::CodeXml,
        }
    }
}

/// (titlebar label, subtitle) — same hints as ManagerRoot.vue.
pub fn meta(view: View) -> (&'static str, &'static str) {
    match view {
        View::Data => ("Данные", "Типы и объекты"),
        View::Sync => ("Синхронизация", "Устройства и связи"),
        View::Packages => ("Маркетплейс", "Приложения и интеграции"),
        View::Engine => ("Движок", "Настройки запуска"),
        View::Settings => ("Настройки", "Запуск Kosmos"),
        View::Connections => ("Интеграции", "Источники данных"),
        View::About => ("О приложении", "Версия и сведения о Kosmos"),
        View::Updates => ("Обновления", "Kosmos Desktop и приложения"),
        View::Secrets => ("Ключи", "API-ключи и провайдеры"),
        View::Browser => ("Браузер", "Сессии и данные сайтов"),
        View::Dev => ("Разработка", "Локальные пакеты и инстанс"),
    }
}

/// Issue the ops a view needs when it becomes active or refreshes.
pub fn load(view: View, app: &mut ManagerApp) {
    match view {
        View::Data => data::load(app),
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
    }
}
