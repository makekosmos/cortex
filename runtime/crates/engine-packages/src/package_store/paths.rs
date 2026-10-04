//! Path-safety rules shared by archive extraction and asset lookup.

use std::path::Path;

pub fn normalize_path(path: &Path) -> Option<String> {
    let value = path.to_string_lossy().replace('\\', "/");
    let value = value.trim_end_matches('/');
    let mut components = Vec::new();
    for component in value.split('/') {
        if component.is_empty()
            || component == "."
            || component == ".."
            || component.contains(':')
            || component.ends_with('.')
            || component.ends_with(' ')
            || is_reserved_name(component)
        {
            return None;
        }
        components.push(component);
    }
    (!components.is_empty()).then(|| components.join("/"))
}

pub(super) fn safe_asset_path(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('/')
        && !value.contains('\\')
        && !value.contains('\0')
        && !value.contains('%')
        && value.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && !part.contains(':')
                && !part.ends_with('.')
                && !part.ends_with(' ')
                && !is_reserved_name(part)
        })
}

pub fn is_reserved_name(component: &str) -> bool {
    let component = component.split('.').next().unwrap_or(component);
    matches!(
        component
            .trim_end_matches(['.', ' '])
            .to_ascii_uppercase()
            .as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}
