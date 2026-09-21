//! Thin wrapper around `uniffi::uniffi_bindgen_main` so the crate can
//! generate Kotlin (and future Swift) bindings via `cargo run --bin
//! uniffi-bindgen`. Not used at runtime by any embedder.

fn main() {
    uniffi::uniffi_bindgen_main();
}
