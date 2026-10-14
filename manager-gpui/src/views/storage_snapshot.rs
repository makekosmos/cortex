use crate::widgets::{varr, vnum, vstr};
use serde_json::{json, Value};

pub(super) fn sort_items(items: &mut [Value]) {
    items.sort_by(|a, b| {
        vnum(b, "bytes")
            .total_cmp(&vnum(a, "bytes"))
            .then_with(|| vstr(a, "id").cmp(&vstr(b, "id")))
            .then_with(|| vstr(a, "label").cmp(&vstr(b, "label")))
            .then_with(|| vstr(a, "path").cmp(&vstr(b, "path")))
    });
}

pub(crate) fn normalize(value: &mut Value) {
    if let Some(categories) = value["categories"].as_array_mut() {
        for category in categories.iter_mut() {
            if let Some(details) = category["detail"].as_array_mut() {
                sort_items(details);
            }
        }
        sort_items(categories);
    }
}

pub(super) fn selected_categories(value: &Value, database_only: bool) -> Vec<Value> {
    let mut categories: Vec<Value> = varr(value, "categories")
        .iter()
        .filter(|category| !database_only || vstr(category, "id") == "database")
        .cloned()
        .collect();
    sort_items(&mut categories);
    categories
}

pub(super) fn segments(value: &Value, database_only: bool) -> Vec<Value> {
    let categories = selected_categories(value, database_only);
    if !database_only {
        return categories;
    }
    let Some(database) = categories.first() else {
        return vec![];
    };
    let mut parts = varr(database, "detail").to_vec();
    if parts.is_empty() {
        return categories;
    }
    // External database files have no table id. Give them stable identities too.
    for (index, part) in parts.iter_mut().enumerate() {
        if vstr(part, "id").is_empty() {
            part["id"] = json!(format!("database_file_{index}"));
        }
    }
    let attributed: f64 = parts.iter().map(|part| vnum(part, "bytes")).sum();
    let remainder = vnum(database, "bytes") - attributed;
    if remainder > 0. {
        parts.push(json!({
            "id":"sqlite_reserved", "label":"Журнал и резерв SQLite", "bytes":remainder,
            "description":"Журнал изменений, память координации и свободные страницы, зарезервированные SQLite."
        }));
    }
    sort_items(&mut parts);
    parts
}
