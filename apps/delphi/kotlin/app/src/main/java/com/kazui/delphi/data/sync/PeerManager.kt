package com.kazui.delphi.data.sync

import android.util.Log
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import com.kazui.delphi.data.space.SpaceManager
import com.kepler.ark.core.ArkCore
import com.kepler.ark.core.ArkEventListener
import com.kepler.ark.core.FfiConnectedPeer
import com.kepler.ark.core.FfiSyncConfig
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import org.json.JSONObject
import javax.inject.Inject
import javax.inject.Singleton

private const val TAG = "PeerManager"

/**
 * Thin coordinator that bridges the Kotlin UI layer to the Rust `ArkCore`
 * sync engine over UniFFI.
 *
 * Every protocol operation (hello / version_vector / batch split / HLC tick
 * / beacon send / peer dedup / self-connect rejection) happens on the Rust
 * side — this file contains no wire format, no WebSocket / UDP code, and no
 * version-vector math. It only:
 *   - Owns a single `ArkCore` instance and forwards start/stop/broadcast
 *     calls to it.
 *   - Installs an `ArkEventListener` implementation that turns Rust events
 *     into the `StateFlow`s + `onDataChanged` callback the Compose layer
 *     already consumes.
 *   - Keeps the pre-migration public API (`broadcastTodoChange`, etc.) so
 *     the UI/view-model layer does not need to change.
 *
 * See `.agent/tasks/ark-rust-runtime/spec.md` AC16 for the removal-of-protocol
 * contract this class fulfils.
 */
@Singleton
class PeerManager @Inject constructor(
    private val spaceManager: SpaceManager,
    private val dataStore: DataStore<Preferences>,
    private val databaseProvider: com.kazui.delphi.di.DatabaseProvider,
) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    @Volatile private var isRunning = false
    private var spaceId: String = ""
    private var deviceId: String = ""
    private var deviceName: String = ""

    private val arkCore: ArkCore = ArkCore()
    @Volatile private var dbOpened = false

    // Exposed state
    private val _connectedPeerCount = MutableStateFlow(0)
    val connectedPeerCount: StateFlow<Int> = _connectedPeerCount.asStateFlow()

    private val _connectedPeerNames = MutableStateFlow<List<String>>(emptyList())
    val connectedPeerNames: StateFlow<List<String>> = _connectedPeerNames.asStateFlow()

    private val _lanSyncState = MutableStateFlow(LanSyncState.DISCONNECTED)
    val lanSyncState: StateFlow<LanSyncState> = _lanSyncState.asStateFlow()

    // Track peer name per device_id, source of truth for _connectedPeerNames.
    private val knownConnectedPeers = linkedMapOf<String, String>()

    // Callback for UI updates when data changes from sync
    var onDataChanged: (() -> Unit)? = null

    // ---------------------------------------------------------------------------
    // Start / Stop
    // ---------------------------------------------------------------------------

    suspend fun start(spaceCode: String, deviceId: String, deviceName: String) {
        if (isRunning) stop()

        val normalizedCode = spaceManager.normalizeCode(spaceCode)
        this.spaceId = spaceManager.deriveSpaceId(normalizedCode)
        this.deviceId = deviceId
        this.deviceName = deviceName
        this.isRunning = true

        Log.i(TAG, "Starting ark-core sync for space=${spaceManager.formatCode(normalizedCode)}")

        // Open the shared DB lazily; the path comes from the DatabaseProvider.
        // TODO: wire the real db path once the Delphi Android DB migration
        // lands. For now the UniFFI binding treats `open_db` as optional
        // when the caller supplies `db_path` inside `FfiSyncConfig`.
        val dbPath = try {
            databaseProvider.arkDbPath()
        } catch (e: Throwable) {
            Log.w(TAG, "DatabaseProvider.arkDbPath() unavailable, sync engine will run DB-less: ${e.message}")
            null
        }

        _lanSyncState.value = LanSyncState.CONNECTING

        try {
            arkCore.startSync(
                FfiSyncConfig(
                    spaceId = spaceId,
                    deviceId = deviceId,
                    deviceName = deviceName,
                    port = SpaceManager.LAN_SYNC_PORT.toUInt(),
                    dbPath = dbPath,
                    seedAddresses = emptyList(),
                ),
                listener = createListener(),
            )
            dbOpened = dbPath != null
            _lanSyncState.value = LanSyncState.LIVE
        } catch (e: Throwable) {
            Log.e(TAG, "startSync failed: ${e.message}", e)
            _lanSyncState.value = LanSyncState.DISCONNECTED
        }
    }

    fun stop() {
        isRunning = false
        try {
            arkCore.stopSync()
        } catch (e: Throwable) {
            Log.w(TAG, "stopSync threw: ${e.message}")
        }
        _connectedPeerCount.value = 0
        _connectedPeerNames.value = emptyList()
        knownConnectedPeers.clear()
        _lanSyncState.value = LanSyncState.DISCONNECTED
        Log.i(TAG, "Stopped")
    }

    // ---------------------------------------------------------------------------
    // Peer connections
    // ---------------------------------------------------------------------------

    suspend fun addPeer(record: PeerRecord) {
        if (!isRunning) return
        addInitialPeer(record.addresses)
    }

    suspend fun addInitialPeer(addresses: List<String>) {
        if (!isRunning) return
        if (addresses.isEmpty()) return
        try {
            arkCore.addSeedPeer(addresses)
        } catch (e: Throwable) {
            Log.w(TAG, "addSeedPeer failed: ${e.message}")
        }
    }

    // ---------------------------------------------------------------------------
    // Broadcast changes
    // ---------------------------------------------------------------------------

    fun broadcastChange(
        entityType: String,
        entityId: String,
        data: JSONObject,
        deleted: Boolean = false,
    ) {
        if (!isRunning) return
        val entity = JSONObject().apply {
            put("type", entityType)
            put("id", entityId)
            put("data", data)
            // HLC is stamped on the Rust side.
            put("hlc", "")
            if (deleted) put("deleted", true)
        }
        scope.launch {
            try {
                arkCore.broadcastChangeJson(entity.toString())
            } catch (e: Throwable) {
                Log.w(TAG, "broadcastChange failed: ${e.message}")
            }
        }
    }

    fun broadcastTodoChange(todo: com.kazui.delphi.data.model.TodoItem) =
        broadcastChange("todo", todo.id, SyncEntityParser.todoToJson(todo))

    fun broadcastTodoDelete(id: String) =
        broadcastChange("todo", id, JSONObject(), deleted = true)

    fun broadcastProjectChange(project: com.kazui.delphi.data.model.Project) =
        broadcastChange("project", project.id, SyncEntityParser.projectToJson(project))

    fun broadcastProjectDelete(id: String) =
        broadcastChange("project", id, JSONObject(), deleted = true)

    fun broadcastAreaChange(area: com.kazui.delphi.data.model.Area) =
        broadcastChange("area", area.id, SyncEntityParser.areaToJson(area))

    fun broadcastAreaDelete(id: String) =
        broadcastChange("area", id, JSONObject(), deleted = true)

    fun broadcastTagChange(tag: com.kazui.delphi.data.model.Tag) =
        broadcastChange("tag", tag.id, SyncEntityParser.tagToJson(tag))

    fun broadcastTagDelete(id: String) =
        broadcastChange("tag", id, JSONObject(), deleted = true)

    fun broadcastHeadingChange(heading: com.kazui.delphi.data.model.Heading) =
        broadcastChange("heading", heading.id, SyncEntityParser.headingToJson(heading))

    fun broadcastHeadingDelete(id: String) =
        broadcastChange("heading", id, JSONObject(), deleted = true)

    // ---------------------------------------------------------------------------
    // Helpers
    // ---------------------------------------------------------------------------

    fun getKnownPeers(): List<PeerRecord> {
        if (!isRunning) return emptyList()
        return try {
            arkCore.getConnectedPeers().map { fp: FfiConnectedPeer ->
                PeerRecord(
                    deviceId = fp.deviceId,
                    deviceName = fp.deviceName,
                    addresses = emptyList(),
                    lastSeen = java.time.Instant.now().toString(),
                )
            }
        } catch (e: Throwable) {
            Log.w(TAG, "getConnectedPeers failed: ${e.message}")
            emptyList()
        }
    }

    private fun createListener(): ArkEventListener = object : ArkEventListener {
        override fun onEntityChanged(entityJson: String) {
            // UI bridge: a change means the local DB has been updated on the
            // Rust side; ask the view-model layer to re-query.
            onDataChanged?.invoke()
        }

        override fun onPeerConnected(deviceId: String, deviceName: String) {
            if (deviceId.isEmpty()) return
            synchronized(knownConnectedPeers) {
                knownConnectedPeers[deviceId] = deviceName
                pushPeerFlowsLocked()
            }
            if (_lanSyncState.value == LanSyncState.CONNECTING ||
                _lanSyncState.value == LanSyncState.DISCONNECTED
            ) {
                _lanSyncState.value = LanSyncState.LIVE
            }
        }

        override fun onPeerDisconnected(deviceId: String, remaining: UInt) {
            synchronized(knownConnectedPeers) {
                knownConnectedPeers.remove(deviceId)
                pushPeerFlowsLocked()
                if (knownConnectedPeers.isEmpty()) {
                    _lanSyncState.value =
                        if (isRunning) LanSyncState.CONNECTING else LanSyncState.DISCONNECTED
                }
            }
        }
    }

    private fun pushPeerFlowsLocked() {
        _connectedPeerCount.value = knownConnectedPeers.size
        _connectedPeerNames.value = knownConnectedPeers.values.toList()
    }
}
