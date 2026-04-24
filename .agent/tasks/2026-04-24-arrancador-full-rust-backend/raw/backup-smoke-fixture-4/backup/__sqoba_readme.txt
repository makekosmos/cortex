SQOBA backup format

This folder contains raw save files plus a manifest.
- __sqoba_manifest.json: list of files and original paths
- files/: backed up files in the same structure as the saves

To restore manually:
1) Open __sqoba_manifest.json
2) For each entry, copy files/<path> to original_path
