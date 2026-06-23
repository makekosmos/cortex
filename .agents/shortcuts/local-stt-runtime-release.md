# Local STT Runtime Release

## Trigger

When publishing Windows `whisper.cpp` CPU/Vulkan runtime zip files for Kosmos local dictation.

## Symptom

The app downloads a runtime zip, SHA passes, but `whisper-cli.exe` is still missing after extraction, or a release build accidentally bundles `resources/local-stt` into the installer.

## Do This

Package runtime zips with forward-slash entries:

```text
Release/whisper-cli.exe
Release/ggml.dll
Release/ggml-base.dll
Release/ggml-cpu.dll
Release/whisper.dll
Release/ggml-vulkan.dll
```

Verify public CDN files before release:

```powershell
Invoke-WebRequest -Uri $url -OutFile $zip
# Check SHA-256 and ensure ZipFile entries include Release/whisper-cli.exe.
```

After updating GitHub Pages assets at the same URL, wait until the CDN returns the new SHA; it can briefly serve stale bytes.

For desktop packaging, verify `platform/desktop/release/win-unpacked/resources/local-stt` is absent and `Kosmos Runtime.exe` / `Kosmos Local STT.exe` are present.

## Avoid

Do not use `.NET ZipFile.CreateFromDirectory` for these runtime zips without checking entries. On Windows it can create `Release\...` paths, which are not the portable archive shape expected by the Rust zip extraction path.

Do not ship a release that points at a 404 or stale runtime URL; `latest.yml` can be valid while local STT installation is still broken.

## Promote To Skill When

This procedure grows to cover multiple runtime families or becomes part of a formal release checklist.
