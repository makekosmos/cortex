# Windows Cargo Target Lock

## Trigger

`cargo test` or `cargo fmt` fails on Windows with `failed to open ... target\debug\.cargo-lock` and `Access is denied`.

## Symptom

The workspace build starts, then Cargo exits before running tests because it cannot open the shared target lock file.

## Do This

Rerun the same Cargo command with escalated permissions so it can touch the workspace target lock:

```powershell
cargo test --manifest-path platform/runtime/Cargo.toml --lib dictation::host::tests::submit_audio_groq_transcript_succeeds_with_test_api_key_and_cleans_up -- --exact --nocapture
```

If the sandbox blocks it again, rerun outside the sandbox rather than changing the command shape.

## Avoid

Do not treat the lock-file denial as a Rust test failure. It is usually a Windows workspace permission issue, not a code regression.

## Promote To Skill When

This keeps happening across multiple Windows workspace repos or needs a more general command recipe.
