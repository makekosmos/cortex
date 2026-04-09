package com.kazui.delphi.data.sync

/**
 * Observable connection state for the LAN sync engine. The previous
 * implementation lived in `LanSyncClient.kt`; after the ark-core / UniFFI
 * migration the enum lives here standalone so the UI layer can depend on it
 * without pulling in the deleted WebSocket client class.
 */
enum class LanSyncState {
    DISCONNECTED,
    CONNECTING,
    CONNECTED,
    SYNCING,
    LIVE,
}
