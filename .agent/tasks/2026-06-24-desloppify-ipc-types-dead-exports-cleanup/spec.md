# ipc-types dead export cleanup

Scope: `platform/desktop/shared/ipc-types.ts`

Goal: remove `export` from local-only IPC type declarations without changing runtime behavior or deleting declarations.

Safe targets:

- `SyncSnapshot`
- `SyncConnectResult`
- `DiagnosticsWindowInfo`
- `DiagnosticsMetricsSnapshot`
- `DiagnosticsWindowMoveBenchmarkInput`
- `DiagnosticsWindowMoveBenchmarkResult`
- `ExtensionInstallPreview`
- `FileSearchRiskLevel`
- `FileIndexLastScanSnapshot`
- `FileIndexDiagnosticsSnapshot`
- `FileSearchRootEstimate`
- `FocusSessionPhase`
- `FocusSessionPomodoroState`

Stopped names:

- `SyncPeerStatus`
- `SyncPeerDeviceKind`
- `SyncPeerInfo`
- `SyncStatusSnapshot`
- `StorageSummaryItem`
- `SearchResult`
- `NtfsStatus`
- `FileIndexSettingsPatch`
- `ClipboardHistorySettingsPatch`
- `FocusActiveState`
- `FocusBlocklist`
