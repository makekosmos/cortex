# Delphi electron API shim cleanup

Remove the redundant warning from `ensureTaskObjectTypeRegistered` catch/rethrow path while preserving retry behavior.

Acceptance:

- `taskObjectTypeReady` is reset on failure.
- The original error is rethrown.
- No extra warning is logged from this catch block.
