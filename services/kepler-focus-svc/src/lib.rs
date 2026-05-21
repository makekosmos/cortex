#![cfg_attr(test, allow(clippy::unwrap_used))]

//! Library facade для kepler-focus-svc — выставляет `protocol` модуль с
//! Request/Response типами и `dispatch` функцией. Unit-тесты для wire format
//! живут здесь (pipe / SCM код требует Windows runtime → не unit-testable).

pub mod protocol;
