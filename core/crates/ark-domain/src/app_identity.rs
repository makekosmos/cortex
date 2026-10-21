/// Produces a stable executable identity across installs that use
/// version-specific directory names.
///
/// `tracked_apps.id` historically included the full executable path. Apps
/// whose executable moves on update then appeared as multiple applications.
/// This key removes unambiguous version-looking path segments while keeping
/// install roots distinct. Plain numeric directory names are retained because
/// they may identify a real install root rather than a version.
pub fn canonical_app_key(normalized_exe_path: &str, process_name: &str) -> String {
    let path = normalized_exe_path.trim().replace('/', "\\").to_lowercase();
    if path.is_empty() {
        return fallback_process_key(process_name);
    }

    let mut output = String::with_capacity(path.len());
    for segment in path.split('\\') {
        if segment.is_empty() || is_version_segment(segment) {
            continue;
        }
        if !output.is_empty() {
            output.push('\\');
        }
        output.push_str(segment);
    }

    if output.is_empty() {
        fallback_process_key(process_name)
    } else {
        output
    }
}

fn fallback_process_key(process_name: &str) -> String {
    format!("proc:{}", process_name.trim().to_lowercase())
}

fn is_version_segment(segment: &str) -> bool {
    let segment = segment.strip_prefix("app-").unwrap_or(segment);
    let segment = segment.strip_prefix('v').unwrap_or(segment);
    let mut chars = segment.chars();
    if !matches!(chars.next(), Some(c) if c.is_ascii_digit()) {
        return false;
    }

    let mut has_separator = false;
    for character in chars {
        match character {
            '0'..='9' => {}
            '.' | '_' | '-' => has_separator = true,
            _ => return false,
        }
    }
    has_separator
}

#[cfg(test)]
mod tests {
    use super::canonical_app_key;

    #[test]
    fn strips_version_directories_but_keeps_install_roots() {
        assert_eq!(
            canonical_app_key("C:/Games/Studio/app-1.0.9164/Game.EXE", "Game.EXE"),
            "c:\\games\\studio\\game.exe"
        );
        assert_eq!(
            canonical_app_key("C:/Games/2024/Game.EXE", "Game.EXE"),
            "c:\\games\\2024\\game.exe"
        );
    }

    #[test]
    fn falls_back_to_a_normalized_process_name_without_a_path() {
        assert_eq!(canonical_app_key("", "  Game.EXE  "), "proc:game.exe");
    }
}
