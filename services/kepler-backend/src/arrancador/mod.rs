// Arrancador backend — scanner + launcher + RAWG + SQOBA модули.
//
// Subagent A scope: `scanner.rs`, `launcher.rs`, `config.rs`. RAWG (`rawg.rs`)
// и SQOBA (`sqoba.rs`) — subagent B / C соответственно. WS-диспатчер
// `handle_arrancador_op` живёт в `ws_server.rs` и регистрирует sub-operations
// `arrancador.scan` + `arrancador.launch` (RAWG / SQOBA — будут зарегистрированы
// другими subagent'ами и сливаются в один match).

pub mod config;
pub mod launcher;
pub mod scanner;
pub mod rawg;
pub mod sqoba;
