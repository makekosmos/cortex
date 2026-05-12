# Problems Found During Verification

Result after fixes: PASS

## Fixed

- Arrancador game object writes failed on a fresh ARK database because `game_obj` could be missing. The service now ensures the object type through the ARK API before writing a game object.
- Arrancador service tests could not reliably import Electron from Node-side test code. The service import was adjusted so tests and runtime both work.
- Eden e2e exposed missing inline caret integration. The custom caret is now mounted in the app and TipTap receives the inline caret extension.
- Eden duplicate title validation was missing from the ARK-backed save path. The store now checks existing ARK note objects before writing.
- Eden e2e selectors/text expectations had drifted from current UI labels and controls. Tests were updated to target the current UI.
- The Eden/Arrancador cross-app test originally depended on direct native `better-sqlite3` loading and hit a Node ABI mismatch in Playwright. The test now exercises the ARK sidecar client and injected ARK object APIs instead.
- Delphi unit tests initially hit sandbox permission failures. The command passed when run through the approved escalation path.

## Non-Blocking Warnings

- Electron CSP warnings are still emitted in smoke/e2e output.
- Vite emits chunk-size and deprecated Rollup `inlineDynamicImports` warnings.
- Electron-builder emits metadata/icon/signing warnings.
- Git reports LF/CRLF normalization warnings.

None of these warnings failed the verification commands.
