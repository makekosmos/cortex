# Manager-owned secrets

## Goal

All user-supplied API keys, tokens and OAuth credentials are entered and managed
only by Kosmos Manager. Products receive configuration status and invoke host
operations; they never receive a secret value.

## Scope

- Move Arcadia RAWG from its plaintext JSON field to Windows Credential Manager.
- Add RAWG management to the Manager Secrets tab.
- Deny credential operations through the extension ARK bridge, including bundled
  and development extensions.
- Keep existing Manager-owned Groq and integrations credential flows intact.

## Acceptance criteria

1. `arrancador-config.json` contains no RAWG key and old values migrate to the
   credential store when available.
2. Arcadia can obtain only the configured/not-configured state and can still run
   RAWG search/apply through the host.
3. An extension cannot invoke a secret read, write, clear or verification operation.
4. Kosmos Manager can add, replace and remove the RAWG key.
5. Relevant Rust, TypeScript and UI checks pass.
