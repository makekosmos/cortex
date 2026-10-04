use super::*;

pub(super) fn assess_roots_risk(roots: &[String]) -> (FileIndexRiskLevel, Vec<String>) {
    let mut level = FileIndexRiskLevel::Ok;
    let mut reasons = Vec::new();
    for root in roots {
        let (root_level, mut root_reasons) = assess_root_path_risk(root);
        level = max_risk(level, root_level);
        reasons.append(&mut root_reasons);
    }
    (level, reasons)
}

pub(super) fn assess_estimate_risk(
    root: &str,
    truncated: bool,
    estimated_entries: usize,
    estimated_size_bytes: u64,
) -> (FileIndexRiskLevel, Vec<String>) {
    let (mut level, mut reasons) = assess_root_path_risk(root);
    if truncated {
        level = max_risk(level, FileIndexRiskLevel::Warning);
        reasons.push("Оценка была усечена по budget/time cap".to_string());
    }
    if estimated_entries >= 500_000 {
        level = max_risk(level, FileIndexRiskLevel::Danger);
        reasons.push(format!(
            "Оценка показывает очень большой объём индекса: {estimated_entries} файлов"
        ));
    } else if estimated_entries >= 100_000 {
        level = max_risk(level, FileIndexRiskLevel::Warning);
        reasons.push(format!(
            "Оценка показывает крупный объём индекса: {estimated_entries} файлов"
        ));
    }
    const MB: u64 = 1024 * 1024;
    if estimated_size_bytes >= 1024 * MB {
        level = max_risk(level, FileIndexRiskLevel::Danger);
        reasons.push(format!(
            "Оценочный размер индекса превышает 1 ГБ: {} байт",
            estimated_size_bytes
        ));
    } else if estimated_size_bytes >= 256 * MB {
        level = max_risk(level, FileIndexRiskLevel::Warning);
        reasons.push(format!(
            "Оценочный размер индекса заметный: {} байт",
            estimated_size_bytes
        ));
    }
    (level, reasons)
}

pub(super) fn assess_root_path_risk(root: &str) -> (FileIndexRiskLevel, Vec<String>) {
    let normalized = root.trim_end_matches(['\\', '/']);
    if normalized.is_empty() {
        return (
            FileIndexRiskLevel::Danger,
            vec!["Пустой или некорректный root индекса".to_string()],
        );
    }
    let mut level = FileIndexRiskLevel::Ok;
    let mut reasons = Vec::new();
    if is_drive_root_like(normalized) || normalized == "/" {
        level = FileIndexRiskLevel::Danger;
        reasons.push(format!(
            "Root `{root}` охватывает корень диска/файловой системы"
        ));
    }
    let lower = normalized.to_ascii_lowercase();
    if lower.ends_with(r":\users")
        || lower.ends_with(r":\users\public")
        || lower.ends_with(r":\programdata")
        || lower.ends_with("/users")
        || lower.ends_with("/home")
    {
        level = max_risk(level, FileIndexRiskLevel::Danger);
        reasons.push(format!(
            "Root `{root}` выглядит слишком широким для постоянного индексирования"
        ));
    }
    let depth = Path::new(normalized)
        .components()
        .filter(|component| matches!(component, std::path::Component::Normal(_)))
        .count();
    if depth <= 1 && !is_drive_root_like(normalized) && normalized != "/" {
        level = max_risk(level, FileIndexRiskLevel::Warning);
        reasons.push(format!(
            "Root `{root}` расположен слишком высоко в дереве каталогов"
        ));
    }
    (level, reasons)
}

pub(super) fn max_risk(a: FileIndexRiskLevel, b: FileIndexRiskLevel) -> FileIndexRiskLevel {
    use FileIndexRiskLevel::{Danger, Ok, Warning};
    match (a, b) {
        (Danger, _) | (_, Danger) => Danger,
        (Warning, _) | (_, Warning) => Warning,
        _ => Ok,
    }
}

pub(super) fn is_drive_root_like(path: &str) -> bool {
    let trimmed = path.trim_end_matches(['\\', '/']);
    let bytes = trimmed.as_bytes();
    bytes.len() == 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic()
}

pub(super) fn indexed_result_is_visible(path: &str, roots: &[String]) -> bool {
    if roots.is_empty() || !Path::new(path).exists() {
        return false;
    }
    roots.iter().any(|root| path_is_under_root(path, root))
}

fn path_is_under_root(path: &str, root: &str) -> bool {
    let normalized_root = root.trim_end_matches(['\\', '/']);
    let normalized_path = path.trim_end_matches(['\\', '/']);
    if normalized_root.is_empty() {
        return false;
    }
    if cfg!(windows) {
        let root_lower = normalized_root.to_ascii_lowercase();
        let path_lower = normalized_path.to_ascii_lowercase();
        path_lower == root_lower
            || path_lower.starts_with(&format!("{root_lower}\\"))
            || path_lower.starts_with(&format!("{root_lower}/"))
    } else {
        normalized_path == normalized_root
            || normalized_path.starts_with(&format!("{normalized_root}/"))
    }
}

#[cfg(test)]
mod tests {
    use super::path_is_under_root;

    #[test]
    fn path_scope_does_not_accept_sibling_prefixes() {
        assert!(path_is_under_root("/safe/root/file.txt", "/safe/root"));
        assert!(!path_is_under_root(
            "/safe/root-escape/file.txt",
            "/safe/root"
        ));
    }
}
