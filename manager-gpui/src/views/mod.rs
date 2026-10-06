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

/// One row per screen: adding a page means a `View` variant, a row here and a
/// slot in `NAV_GROUPS`.
struct Page {
    view: View,
    key: &'static str,
    label: &'static str,
    icon: &'static str,
    load: fn(&mut ManagerApp),
    render: fn(&mut ManagerApp, &mut Window, &mut Context<ManagerApp>) -> AnyElement,
}

const PAGES: &[Page] = &[
    Page {
        view: View::Data,
        key: "data",
        label: "Данные",
        icon: "icons/database.svg",
        load: data::load,
        render: data::render,
    },
    Page {
        view: View::Usage,
        key: "usage",
        label: "Активность",
        icon: "icons/cpu.svg",
        load: usage::load,
        render: usage::render,
    },
    Page {
        view: View::Sync,
        key: "sync",
        label: "Девайсы",
        icon: "icons/device-monitor.svg",
        load: sync::load,
        render: sync::render,
    },
    Page {
        view: View::Packages,
        key: "packages",
        label: "Приложения",
        icon: "icons/store.svg",
        load: store::load,
        render: store::render,
    },
    Page {
        view: View::Settings,
        key: "settings",
        label: "Настройки",
        icon: "icons/settings.svg",
        load: settings::load,
        render: settings::render,
    },
    Page {
        view: View::Appearance,
        key: "appearance",
        label: "Внешний вид",
        icon: "icons/appearance.svg",
        load: appearance::load,
        render: appearance::render,
    },
    Page {
        view: View::Browser,
        key: "browser",
        label: "Браузер",
        icon: "icons/globe.svg",
        load: browser::load,
        render: browser::render,
    },
    Page {
        view: View::Connections,
        key: "connections",
        label: "Интеграции",
        icon: "icons/connections.svg",
        load: connections::load,
        render: connections::render,
    },
    Page {
        view: View::Keys,
        key: "keys",
        label: "Ключи",
        icon: "icons/key.svg",
        load: secrets::load,
        render: secrets::render,
    },
    Page {
        view: View::About,
        key: "about",
        label: "О приложении",
        icon: "icons/help-circle.svg",
        load: about::load,
        render: about::render,
    },
    Page {
        view: View::Dev,
        key: "dev",
        label: "Разработчикам",
        icon: "icons/code.svg",
        load: dev::load,
        render: dev::render,
    },
];

impl View {
    fn page(self) -> &'static Page {
        PAGES
            .iter()
            .find(|page| page.view == self)
            .expect("every View has a PAGES row")
    }

    pub fn label(self) -> &'static str {
        self.page().label
    }

    /// Stable persistence key for the last-opened page.
    pub fn key(self) -> &'static str {
        self.page().key
    }

    pub fn from_key(key: &str) -> Option<View> {
        // "updates" was the Обновления page before it merged into О приложении.
        let key = if key == "updates" { "about" } else { key };
        PAGES
            .iter()
            .find(|page| page.key == key)
            .map(|page| page.view)
    }

    pub fn icon(self) -> Icon {
        Icon::default().path(self.page().icon)
    }
}

/// Issue the ops a view needs when it becomes active or refreshes.
pub fn load(view: View, app: &mut ManagerApp) {
    (view.page().load)(app);
}

pub fn render(
    view: View,
    app: &mut ManagerApp,
    window: &mut Window,
    cx: &mut Context<ManagerApp>,
) -> AnyElement {
    (view.page().render)(app, window, cx)
}

#[cfg(test)]
mod tests {
    use super::{View, PAGES};

    #[test]
    fn every_view_has_exactly_one_page_with_a_unique_key() {
        let views = [
            View::Data,
            View::Usage,
            View::Sync,
            View::Packages,
            View::Settings,
            View::Appearance,
            View::Browser,
            View::Connections,
            View::Keys,
            View::About,
            View::Dev,
        ];
        assert_eq!(PAGES.len(), views.len());
        for view in views {
            assert_eq!(PAGES.iter().filter(|p| p.view == view).count(), 1);
            assert_eq!(View::from_key(view.key()), Some(view));
        }
        assert_eq!(View::from_key("updates"), Some(View::About));
        assert_eq!(View::from_key("nope"), None);
    }
}
