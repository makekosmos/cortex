//! `hosts.sync`: write a named managed block into the hosts file.
//!
//! This is the OS capability, not a feature. The caller picks the block name
//! and declares the domains it wants blocked; an empty list removes the block.
//! The privileged service owns the file and only edits lines inside its
//! `# === engine:<name> BEGIN/END ===` markers.

use serde_json::Value;

pub fn is_hosts_operation(operation: &str) -> bool {
    operation == "hosts.sync"
}

/// Apply `hosts.sync`. Names follow the service's block-name rule, so a block
/// can never escape its own markers; domains are validated here because the
/// service writes them verbatim.
pub fn sync(params: &Value) -> Result<Value, String> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or("hosts.sync: missing 'name'")?;
    if !is_valid_block_name(name) {
        return Err(format!(
            "hosts.sync: block name {name:?} must be 1-32 of [a-z0-9-]"
        ));
    }
    let domains = match params.get("domains") {
        Some(Value::Array(entries)) => entries
            .iter()
            .map(|entry| {
                entry
                    .as_str()
                    .ok_or_else(|| "hosts.sync: 'domains' must be strings".to_string())
            })
            .collect::<Result<Vec<_>, _>>()?,
        Some(_) => return Err("hosts.sync: 'domains' must be an array".into()),
        None => return Err("hosts.sync: missing 'domains'".into()),
    };
    let domains = domains
        .iter()
        .map(|domain| validate_domain(domain))
        .collect::<Result<Vec<_>, _>>()?;

    crate::privileged::client::sync_managed_block(name, &domains)?;
    Ok(serde_json::json!({
        "name": name,
        "domains": domains.len(),
        "applied": !domains.is_empty(),
    }))
}

fn is_valid_block_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 32
        && bytes
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
}

/// Trim, lowercase, and require a dot-separated hostname with a real TLD.
fn validate_domain(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("hosts.sync: domain cannot be empty".into());
    }
    let lower = trimmed.to_ascii_lowercase();
    for byte in lower.bytes() {
        let ok = byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'.' || byte == b'-';
        if !ok {
            return Err(format!("hosts.sync: invalid character in domain {raw:?}"));
        }
    }
    let last_dot = lower
        .rfind('.')
        .ok_or_else(|| format!("hosts.sync: domain {raw:?} has no TLD"))?;
    if last_dot == 0 || last_dot == lower.len() - 1 {
        return Err(format!(
            "hosts.sync: domain {raw:?} has an empty label next to a dot"
        ));
    }
    let tld = &lower[last_dot + 1..];
    if tld.len() < 2 || !tld.bytes().all(|byte| byte.is_ascii_lowercase()) {
        return Err(format!(
            "hosts.sync: domain {raw:?} TLD must be >=2 letters"
        ));
    }
    if lower[..last_dot].split('.').any(str::is_empty) {
        return Err(format!("hosts.sync: domain {raw:?} has an empty label"));
    }
    Ok(lower)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    type Calls = Arc<Mutex<Vec<(String, Vec<String>)>>>;
    type Delegate = Arc<dyn Fn(&str, &[String]) -> Result<(), String> + Send + Sync>;

    /// The delegate is process-global, so these tests must not overlap.
    static DELEGATE_LOCK: Mutex<()> = Mutex::new(());

    fn recording_delegate() -> (Calls, Delegate) {
        let calls: Calls = Arc::new(Mutex::new(Vec::new()));
        let recorded = calls.clone();
        let delegate = Arc::new(move |name: &str, domains: &[String]| {
            recorded
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .push((name.to_string(), domains.to_vec()));
            Ok(())
        });
        (calls, delegate)
    }

    #[test]
    fn hosts_sync_writes_and_clears_a_named_block() {
        let _guard = DELEGATE_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let (calls, delegate) = recording_delegate();
        crate::privileged::client::set_hosts_delegate_for_test(Some(delegate));

        let applied = sync(&serde_json::json!({
            "name": "site-block",
            "domains": ["Example.COM ", "news.example.org"],
        }))
        .expect("the block is written");
        assert_eq!(applied["applied"], serde_json::json!(true));
        assert_eq!(applied["domains"], serde_json::json!(2));

        let cleared = sync(&serde_json::json!({ "name": "site-block", "domains": [] }))
            .expect("an empty list removes the block");
        assert_eq!(cleared["applied"], serde_json::json!(false));

        assert_eq!(
            *calls.lock().unwrap_or_else(|error| error.into_inner()),
            vec![
                (
                    "site-block".to_string(),
                    vec!["example.com".to_string(), "news.example.org".to_string()]
                ),
                ("site-block".to_string(), Vec::new()),
            ]
        );
        crate::privileged::client::set_hosts_delegate_for_test(None);
    }

    #[test]
    fn hosts_sync_rejects_unusable_input() {
        let _guard = DELEGATE_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let (calls, delegate) = recording_delegate();
        crate::privileged::client::set_hosts_delegate_for_test(Some(delegate));

        for params in [
            serde_json::json!({ "domains": ["example.com"] }),
            serde_json::json!({ "name": "site-block" }),
            serde_json::json!({ "name": "Site Block", "domains": [] }),
            serde_json::json!({ "name": "site-block", "domains": "example.com" }),
            serde_json::json!({ "name": "site-block", "domains": ["localhost"] }),
            serde_json::json!({ "name": "site-block", "domains": ["bad domain.com"] }),
            serde_json::json!({ "name": "site-block", "domains": ["example.com\n127.0.0.1 bank.example"] }),
        ] {
            assert!(sync(&params).is_err(), "{params} must be rejected");
        }
        assert!(calls
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .is_empty());
        crate::privileged::client::set_hosts_delegate_for_test(None);
    }
}
