# Evidence: ARK object FTS5 search

## Result

PASS

## Acceptance Criteria

AC1. PASS. `init_schema` creates `object_search_fts` when FTS5 is available and rebuilds it for existing objects.

AC2. PASS. `upsert_object` updates the object row and FTS entry inside a savepoint.

AC3. PASS. `delete_object` removes the object row and FTS entry inside a savepoint.

AC4. PASS. `search_objects` uses FTS5 `MATCH` for tokenized non-empty queries and keeps the existing `SearchResult` shape.

AC5. PASS. Search query tokens are built from alphanumeric terms, punctuation-heavy input is safe, and search falls back when FTS is unavailable or unusable.

AC6. PASS. Added tests for schema-init rebuild, update/delete indexing, punctuation-safe search, and fallback without FTS.

AC7. PASS. Fresh checks were run against the current codebase.

## Raw Artifacts

- `cargo-fmt-check.txt`
- `cargo-check.txt`
- `cargo-test.txt`
- `git-diff-check.txt`
- `final-cargo-fmt-check.txt`
- `final-cargo-check.txt`
- `final-cargo-test.txt`
- `final-git-diff-check.txt`
- `post-warning-cleanup-cargo-fmt-check.txt`
- `post-warning-cleanup-cargo-test.txt`
- `post-warning-cleanup-git-diff-check.txt`
- `diff.patch`

## Verification Commands

- `cargo fmt --manifest-path packages\ark-core\rust\Cargo.toml -- --check`
- `cargo check --manifest-path packages\ark-core\rust\Cargo.toml`
- `cargo test --manifest-path packages\ark-core\rust\Cargo.toml`
- `git diff --check -- packages\ark-core\rust\src\db.rs packages\ark-core\rust\src\schema.rs`

## Notes

`cargo test` passes. It reports existing warnings in `tests\relay_round_trip.rs` for unused imports/variables, but exits successfully.

`git diff --check` exits successfully. It reports the existing Windows line-ending notice that LF will be replaced by CRLF when Git touches the edited files.

The final fresh verification pass after writing this evidence also passed. After a small relay test warning cleanup, `cargo fmt --check`, full `cargo test`, and scoped `git diff --check` were run again and passed without Rust warnings.
