# Verification Problems

## P1: Relay tests used invalid todo payloads

Initial focused relay tests failed before exercising relay behavior because the test `todo_entity` helper only included `title`. `SqliteStorageBackend::apply_entity` validates todo payloads and requires fields such as `priority`.

### Fix

Use a complete valid todo payload in relay tests, matching the existing sync integration tests. No runtime relay logic was changed for this issue.
