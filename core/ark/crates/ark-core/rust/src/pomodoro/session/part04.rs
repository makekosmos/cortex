#[cfg(test)]
mod tests {
    use super::*;
    use crate::pomodoro::clock::MockClock;
    use std::sync::Arc;
    include!("tests/part01.rs");
    include!("tests/part02.rs");
}
