use super::*;

mod grants;
mod legacy;

fn owner(n: u64) -> GrantOwner {
    GrantOwner {
        session_id: "s".into(),
        generation: n,
        connection_id: n,
    }
}
