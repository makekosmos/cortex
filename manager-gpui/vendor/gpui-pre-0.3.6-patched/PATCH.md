# Local patch vs crates.io gpui-pre 0.3.6

Vendored copy of the crates.io `gpui-pre` 0.3.6 sources (the registry
`Cargo.toml`/`Cargo.toml.orig` pair is kept verbatim). Carries one local
patch for KOS-142 accessibility-tree tests.

### TestWindow: activate the a11y pipeline in tests

Test windows create no platform adapter, so `PlatformWindow::a11y_init` was
never invoked and `Window::debug_a11y_tree_json` always returned `None` in
`#[gpui::test]` runs. `TestWindow::a11y_init` now fires the `activation`
callback once — the same thing a screen reader connecting does — which sets
the shared active flag. `A11y::sync_active_flag` then collects accesskit
nodes every frame, so snapshot tests can dump and assert the real
accessibility tree.

Files changed:
- src/platform/test/window.rs — `impl PlatformWindow for TestWindow` gains an
  `a11y_init` override that calls `callbacks.activation()` immediately, plus
  the `A11yCallbacks` import. The test platform only compiles under
  `cfg(any(test, feature = "test-support"))`, so production windows are
  untouched.
