#[cfg(test)]
mod tests {
    use super::*;
    use crate::pomodoro::clock::MockClock;
    use std::sync::Arc;

    include!("tests/transitions.rs");
    include!("tests/restore.rs");
}
