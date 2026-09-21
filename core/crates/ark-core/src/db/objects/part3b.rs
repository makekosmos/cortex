
fn line_matches_query(line: &str, normalized_query: &str, query_terms: &[String]) -> bool {
    if line.is_empty() {
        return false;
    }

    let line_lower = line.to_lowercase();
    if line_lower.contains(normalized_query) {
        return true;
    }

    let line_terms = tokenize_search_text(line);
    query_terms.iter().any(|query_term| {
        line_terms
            .iter()
            .any(|line_term| line_term.starts_with(query_term))
    })
}

fn build_snippet(line: &str, normalized_query: &str, query_terms: &[String]) -> String {
    let line_lower = line.to_lowercase();
    let direct_match = line_lower.find(normalized_query);
    let term_match = query_terms.iter().find_map(|term| line_lower.find(term));
    let match_start = direct_match.or(term_match).unwrap_or(0);
    let snippet_radius = 56;

    let start = char_boundary_before(line, match_start.saturating_sub(snippet_radius));
    let end = char_boundary_after(
        line,
        (match_start + normalized_query.len() + snippet_radius).min(line.len()),
    );
    let snippet = line[start..end].trim();

    if start == 0 && end == line.len() {
        truncate_snippet(snippet, 140)
    } else {
        let mut result = String::new();
        if start > 0 {
            result.push_str("...");
        }
        result.push_str(snippet);
        if end < line.len() {
            result.push_str("...");
        }
        result
    }
}

fn truncate_snippet(line: &str, max_chars: usize) -> String {
    if line.chars().count() <= max_chars {
        return line.to_string();
    }

    let truncated = line.chars().take(max_chars).collect::<String>();
    format!("{}...", truncated.trim_end())
}

fn char_boundary_before(value: &str, index: usize) -> usize {
    let mut safe_index = index.min(value.len());
    while safe_index > 0 && !value.is_char_boundary(safe_index) {
        safe_index -= 1;
    }
    safe_index
}

fn char_boundary_after(value: &str, index: usize) -> usize {
    let mut safe_index = index.min(value.len());
    while safe_index < value.len() && !value.is_char_boundary(safe_index) {
        safe_index += 1;
    }
    safe_index.min(value.len())
}

fn tokenize_search_text(value: &str) -> Vec<String> {
    value
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(|part| part.to_lowercase())
        .collect()
}

fn extract_plain_text_from_value(value: &Value) -> String {
    let mut output = String::new();
    collect_plain_text(value, &mut output);
    output.trim().to_string()
}

fn collect_plain_text(node: &Value, output: &mut String) {
    if let Some(node_type) = node.get("type").and_then(|value| value.as_str()) {
        match node_type {
            "text" => {
                if let Some(text) = node.get("text").and_then(|value| value.as_str()) {
                    output.push_str(text);
                    output.push(' ');
                }
            }
            "hardBreak" => output.push('\n'),
            _ => {}
        }
    }

    if let Some(text) = node.as_str() {
        output.push_str(text);
        output.push(' ');
        return;
    }

    if let Some(children) = node.get("content").and_then(|value| value.as_array()) {
        for child in children {
            collect_plain_text(child, output);
        }
    }

    if let Some(values) = node.as_array() {
        for value in values {
            collect_plain_text(value, output);
        }
    }

    if let Some(values) = node.as_object() {
        let is_rich_text_text_node = matches!(
            node.get("type").and_then(|value| value.as_str()),
            Some("text")
        );
        for (key, value) in values {
            if key == "type" || key == "content" || (key == "text" && is_rich_text_text_node) {
                continue;
            }
            collect_plain_text(value, output);
        }
    }

    if matches!(
        node.get("type").and_then(|value| value.as_str()),
        Some("paragraph" | "heading" | "codeBlock" | "blockquote" | "listItem")
    ) {
        output.push('\n');
    }
}
