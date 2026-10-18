fn search_objects_with_fts(
    conn: &Connection,
    fts_query: &str,
    normalized_query: &str,
    query_terms: &[String],
) -> Result<Vec<SearchResult>, String> {
    if !object_search_fts_exists(conn)? {
        return Err("object_search_fts is not available".to_string());
    }

    let mut stmt = conn
        .prepare(
            "SELECT objects.id, objects.type_id, objects.type_version, objects.title,
             objects.content_json,  objects.props_json
             FROM object_search_fts
             JOIN objects ON objects.id = object_search_fts.object_id
             WHERE object_search_fts MATCH ?1
             AND objects.deleted_at IS NULL
             ORDER BY bm25(object_search_fts), objects.updated_at DESC, objects.created_at DESC
             LIMIT 30",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![fts_query], |row| {
            let content_json: String = row.get(4)?;
            let props_json: String = row.get(5)?;
            Ok(ArkObject {
                id: row.get(0)?,
                type_id: row.get(1)?,
                type_version: row.get(2)?,
                title: row.get(3)?,
                content_json: parse_json_or_default(content_json),
                props_json: parse_json_or_default(props_json),
                created_at: String::new(),
                updated_at: String::new(),
                deleted_at: None,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut results = Vec::new();
    for object in rows {
        let object = object.map_err(|e| e.to_string())?;
        let body = object_search_body(&object);
        let (line, text) =
            build_search_context(&object.title, &body, normalized_query, query_terms);
        results.push(SearchResult {
            file: String::new(),
            line,
            text,
            entry_id: object.id,
        });
    }
    Ok(results)
}

fn search_objects_fallback(
    conn: &Connection,
    normalized_query: &str,
    query_terms: &[String],
) -> Result<Vec<SearchResult>, String> {
    let mut objects = list_objects(conn)?;
    objects.retain(|object| object.deleted_at.is_none());

    let mut matches = Vec::new();

    for object in objects {
        let body = object_search_body(&object);
        let title_matches = line_matches_query(&object.title, normalized_query, query_terms);
        let body_matches = body
            .lines()
            .any(|line| line_matches_query(line.trim(), normalized_query, query_terms));

        if !title_matches && !body_matches {
            continue;
        }

        let title_lower = object.title.to_lowercase();
        let body_lower = body.to_lowercase();
        let score = if title_lower.contains(normalized_query) {
            400
        } else if title_matches {
            300
        } else if body_lower.contains(normalized_query) {
            200
        } else {
            100
        };

        let (line, text) =
            build_search_context(&object.title, &body, normalized_query, query_terms);
        matches.push((
            score,
            SearchResult {
                file: String::new(),
                line,
                text,
                entry_id: object.id,
            },
        ));
    }

    matches.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
    matches.truncate(30);

    Ok(matches.into_iter().map(|(_, result)| result).collect())
}

fn build_fts_match_query(query_terms: &[String]) -> Option<String> {
    if query_terms.is_empty() {
        return None;
    }

    let terms = query_terms
        .iter()
        .map(|term| format!("{term}*"))
        .collect::<Vec<_>>();
    Some(terms.join(" AND "))
}

fn object_search_body(object: &ArkObject) -> String {
    let content = extract_plain_text_from_value(&object.content_json);
    let props = extract_plain_text_from_value(&object.props_json);

    match (content.is_empty(), props.is_empty()) {
        (true, true) => String::new(),
        (false, true) => content,
        (true, false) => props,
        (false, false) => format!("{content}\n{props}"),
    }
}

fn build_search_context(
    title: &str,
    body: &str,
    normalized_query: &str,
    query_terms: &[String],
) -> (usize, String) {
    for (index, line) in body.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if line_matches_query(trimmed, normalized_query, query_terms) {
            return (
                index + 1,
                build_snippet(trimmed, normalized_query, query_terms),
            );
        }
    }

    if line_matches_query(title, normalized_query, query_terms) {
        return (
            0,
            format!(
                "РќР°Р·РІР°РЅРёРµ: {}",
                build_snippet(title.trim(), normalized_query, query_terms)
            ),
        );
    }

    let first_line = body
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default();

    if !first_line.is_empty() {
        return (1, truncate_snippet(first_line, 140));
    }

    (0, format!("РќР°Р·РІР°РЅРёРµ: {}", title.trim()))
}
