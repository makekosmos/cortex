include!("db/open.rs");
include!("db/legacy.rs");
include!("db/objects.rs");
include!("db/usage.rs");
include!("db/sync.rs");
include!("db/load.rs");
include!("db/backend.rs");

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    include!("db/tests_core.rs");
    include!("db/tests_sync.rs");
}
