//! App-specific Hugeicons, with Imago's existing icons/fonts as fallback.
use gpui::{AssetSource, Result, SharedString};
use std::borrow::Cow;

pub struct Assets;
const ICONS: &[(&str, &[u8])] = &[
    (
        "icons/appearance.svg",
        include_bytes!("../assets/icons/appearance.svg"),
    ),
    (
        "icons/device-laptop.svg",
        include_bytes!("../assets/icons/device-laptop.svg"),
    ),
    (
        "icons/device-monitor.svg",
        include_bytes!("../assets/icons/device-monitor.svg"),
    ),
    (
        "icons/device-android.svg",
        include_bytes!("../assets/icons/device-android.svg"),
    ),
    (
        "icons/device-iphone.svg",
        include_bytes!("../assets/icons/device-iphone.svg"),
    ),
];

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some((_, bytes)) = ICONS.iter().find(|(name, _)| *name == path) {
            return Ok(Some(Cow::Borrowed(*bytes)));
        }
        imago_gpui::assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut paths = imago_gpui::assets::Assets.list(path)?;
        paths.extend(
            ICONS
                .iter()
                .filter(|(name, _)| name.starts_with(path))
                .map(|(name, _)| SharedString::new_static(name)),
        );
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
}

#[cfg(test)]
mod tests {
    use super::{AssetSource, Assets, ICONS};

    #[test]
    fn device_and_existing_icons_are_embedded() {
        for (name, _) in ICONS {
            let bytes = Assets.load(name).unwrap().unwrap();
            let svg = std::str::from_utf8(&bytes).unwrap();
            assert!(svg.contains("viewBox=\"0 0 24 24\""));
            assert!(svg.contains("stroke-width=\"1.5\""), "{name}");
        }
        assert!(Assets.load("icons/sidebar-left.svg").unwrap().is_some());
        assert!(Assets.list("icons/device-").unwrap().len() == 4);
    }
}
