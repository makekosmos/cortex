# Problems: Packaged smoke checks for Arrancador and Dashboard

No unresolved problems.

## Notes

- Initial sandboxed smoke runs failed with `spawn EPERM` while starting native child processes.
- The Dashboard smoke passed when rerun outside the sandbox.
- The first Arrancador outside-sandbox attempt reached electron-builder but failed extracting the Windows code-sign cache because the current Windows session cannot create the symlinks in that archive.
- The Arrancador smoke script now uses `electron-builder --dir -c.win.signAndEditExecutable=false`, which is appropriate for unpacked smoke because it avoids installer/code-sign behavior while still producing and launching the unpacked app.
