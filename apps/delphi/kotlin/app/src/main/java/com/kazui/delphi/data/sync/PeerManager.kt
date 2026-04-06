package com.kazui.delphi.data.sync

import android.util.Log
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import com.kazui.delphi.data.space.SpaceManager
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import org.json.JSONObject
import java.time.Instant
import javax.inject.Inject
import javax.inject.Singleton

private const val TAG = "PeerManager"
private const val RECONNECT_DELAY_MS = 5_000L

/**
 * Coordinates the WS server + WS clients for equal-peer P2P sync.
 *
 * Every device runs:
 *   - SyncServer (Ktor, listens on port 21531)
 *   - LanSyncClient connections to all known peers
 *
 * PeerManager orchestrates both, manages the peer list, and handles
 * peer_list exchange for mesh discovery.
 */
@Singleton
class PeerManager @Inject constructor(
    private val syncServer: SyncServer,
    private val lanSyncClient: LanSyncClient,
    private val spaceManager: SpaceManager,
    private val dataStore: DataStore<Preferences>,
) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    @Volatile private var isRunning = false
    private var spaceCode: String = ""
    private var spaceId: String = ""
    private var deviceId: String = ""
    private var deviceName: String = ""

    private val broadcastDiscovery = BroadcastDiscovery(scope)
    private val reconnectJobs = mutableMapOf<String, Job>()

    // Exposed state
    private val _connectedPeerCount = MutableStateFlow(0)
    val connectedPeerCount: StateFlow<Int> = _connectedPeerCount.asStateFlow()

    private val _connectedPeerNames = MutableStateFlow<List<String>>(emptyList())
    val connectedPeerNames: StateFlow<List<String>> = _connectedPeerNames.asStateFlow()

    /** Combined sync state: LIVE if any connection is live. */
    val lanSyncState: StateFlow<LanSyncState> = lanSyncClient.state

    // Callback for UI updates when data changes from sync
    var onDataChanged: (() -> Unit)? = null

    // ---------------------------------------------------------------------------
    // Start / Stop
    // ---------------------------------------------------------------------------

    suspend fun start(spaceCode: String, deviceId: String, deviceName: String) {
        if (isRunning) stop()

        this.spaceCode = spaceManager.normalizeCode(spaceCode)
        this.spaceId = spaceManager.deriveSpaceId(this.spaceCode)
        this.deviceId = deviceId
        this.deviceName = deviceName
        this.isRunning = true

        val ownAddresses = spaceManager.getOwnAddresses()
        Log.i(TAG, "Starting with space=${spaceManager.formatCode(this.spaceCode)}")

        // Wire up server callbacks
        syncServer.onChangeReceived = { entity ->
            onDataChanged?.invoke()
        }
        syncServer.onPeerConnected = { peerDeviceId, peerName ->
            updatePeerCounts()
            Log.i(TAG, "Server: peer connected: $peerName ($peerDeviceId)")
        }
        syncServer.onPeerDisconnected = { peerDeviceId ->
            updatePeerCounts()
            Log.i(TAG, "Server: peer disconnected: $peerDeviceId")
        }
        syncServer.onNewPeerDiscovered = { record ->
            // Try connecting to newly discovered peer
            scope.launch {
                connectToPeer(record)
            }
        }

        // Wire up client callbacks
        lanSyncClient.onDataChanged = {
            onDataChanged?.invoke()
        }
        lanSyncClient.onPeerListReceived = { peerList ->
            scope.launch {
                handlePeerListFromClient(peerList)
            }
        }

        // Start the WS server
        try {
            syncServer.start(
                port = SpaceManager.LAN_SYNC_PORT,
                spaceId = spaceId,
                deviceId = deviceId,
                deviceName = deviceName,
                ownAddresses = ownAddresses,
            )
        } catch (e: Exception) {
            Log.e(TAG, "Failed to start sync server: ${e.message}")
            // Continue anyway -- we can still work as a client
        }

        // Migrate legacy lan_sync_ip → peer record
        val prefs = dataStore.data.first()
        val legacyIp = prefs[stringPreferencesKey("lan_sync_ip")]
        if (!legacyIp.isNullOrBlank()) {
            val legacyPeer = PeerRecord(
                deviceId = "legacy-electron",
                deviceName = "Delphi Electron",
                addresses = listOf("$legacyIp:21531"),
                lastSeen = Instant.now().toString(),
            )
            syncServer.addKnownPeer(legacyPeer)
            // Clear legacy key
            dataStore.edit { it.remove(stringPreferencesKey("lan_sync_ip")) }
        }

        // Connect to all known peers as client
        val knownPeers = syncServer.getKnownPeerRecords()
        for (peer in knownPeers) {
            if (peer.deviceId == deviceId) continue
            if (syncServer.isConnectedTo(peer.deviceId)) continue
            scope.launch {
                connectToPeer(peer)
            }
        }

        // Start UDP broadcast discovery for automatic peer re-discovery
        val actualPort = syncServer.actualPort.takeIf { it > 0 } ?: SpaceManager.LAN_SYNC_PORT
        broadcastDiscovery.onPeerDiscovered = { beaconPeer ->
            Log.i(TAG, "Beacon from ${beaconPeer.deviceName} at ${beaconPeer.address}")
            scope.launch {
                // Update peer record with fresh address
                syncServer.registerExternalPeer(beaconPeer.deviceId, beaconPeer.deviceName, listOf(beaconPeer.address))

                // Connect if not already connected
                if (!syncServer.isConnectedTo(beaconPeer.deviceId)) {
                    val record = PeerRecord(
                        deviceId = beaconPeer.deviceId,
                        deviceName = beaconPeer.deviceName,
                        addresses = listOf(beaconPeer.address),
                        lastSeen = Instant.now().toString(),
                    )
                    connectToPeer(record)
                }
            }
        }
        broadcastDiscovery.start(spaceId, deviceId, deviceName, actualPort)
    }

    fun stop() {
        isRunning = false
        broadcastDiscovery.stop()
        reconnectJobs.values.forEach { it.cancel() }
        reconnectJobs.clear()
        lanSyncClient.disconnect()
        scope.launch { syncServer.stop() }
        _connectedPeerCount.value = 0
        _connectedPeerNames.value = emptyList()
        Log.i(TAG, "Stopped")
    }

    // ---------------------------------------------------------------------------
    // Peer connections
    // ---------------------------------------------------------------------------

    /**
     * Add a peer and try connecting to it.
     * Used when scanning QR or receiving peer_list.
     */
    suspend fun addPeer(record: PeerRecord) {
        syncServer.registerExternalPeer(record.deviceId, record.deviceName, record.addresses)
        if (!syncServer.isConnectedTo(record.deviceId) && lanSyncClient.state.value == LanSyncState.DISCONNECTED) {
            connectToPeer(record)
        }
    }

    /**
     * Add an initial peer by addresses (for QR join flow).
     */
    suspend fun addInitialPeer(addresses: List<String>) {
        // We don't know device_id yet -- connect and learn it from hello
        if (addresses.isEmpty()) return

        val ownAddresses = spaceManager.getOwnAddresses()

        // Try each address, connecting to the first one that works
        for (addr in addresses) {
            val ip = extractIpFromAddress(addr) ?: continue
            lanSyncClient.connect(ip, spaceId, deviceId, deviceName, ownAddresses)
            return
        }
    }

    private suspend fun connectToPeer(record: PeerRecord) {
        if (!isRunning) return
        if (record.deviceId == deviceId) return

        // Don't reconnect if already connected or connecting as client
        val clientState = lanSyncClient.state.value
        if (clientState == LanSyncState.LIVE || clientState == LanSyncState.SYNCING || clientState == LanSyncState.CONNECTED || clientState == LanSyncState.CONNECTING) {
            return
        }

        // Sort addresses: lastAddress first, then LAN IPs, then others
        val sortedAddresses = buildList {
            record.lastAddress?.let { add(it) }
            addAll(record.addresses.filter { it != record.lastAddress })
        }

        val ownAddresses = spaceManager.getOwnAddresses()

        // Try addresses sequentially (first success wins)
        for (addr in sortedAddresses) {
            if (!isRunning) return
            val ip = extractIpFromAddress(addr) ?: continue
            lanSyncClient.connect(ip, spaceId, deviceId, deviceName, ownAddresses)
            return
        }
    }

    private fun scheduleReconnect(record: PeerRecord) {
        if (!isRunning) return
        if (reconnectJobs.containsKey(record.deviceId)) return
        reconnectJobs[record.deviceId] = scope.launch {
            delay(RECONNECT_DELAY_MS)
            reconnectJobs.remove(record.deviceId)
            if (isRunning && !syncServer.isConnectedTo(record.deviceId)) {
                connectToPeer(record)
            }
        }
    }

    // ---------------------------------------------------------------------------
    // Broadcast changes
    // ---------------------------------------------------------------------------

    /** Broadcast a local entity change to all connected peers. */
    fun broadcastChange(entityType: String, entityId: String, data: JSONObject, deleted: Boolean = false) {
        // Broadcast via client (if connected as client to a server)
        lanSyncClient.sendLiveChange(entityType, entityId, data, deleted)

        // Broadcast via server (to all connected clients)
        scope.launch {
            val hlc = syncServer.updateEntityHlc(entityId)
            val entity = JSONObject().apply {
                put("type", entityType)
                put("id", entityId)
                put("data", data)
                put("hlc", hlc)
                if (deleted) put("deleted", true)
            }
            syncServer.broadcastLiveChange(entity)
        }
    }

    fun broadcastTodoChange(todo: com.kazui.delphi.data.model.TodoItem) {
        broadcastChange("todo", todo.id, SyncEntityParser.todoToJson(todo))
    }

    fun broadcastTodoDelete(id: String) {
        broadcastChange("todo", id, JSONObject(), deleted = true)
    }

    fun broadcastProjectChange(project: com.kazui.delphi.data.model.Project) {
        broadcastChange("project", project.id, SyncEntityParser.projectToJson(project))
    }

    fun broadcastProjectDelete(id: String) {
        broadcastChange("project", id, JSONObject(), deleted = true)
    }

    fun broadcastAreaChange(area: com.kazui.delphi.data.model.Area) {
        broadcastChange("area", area.id, SyncEntityParser.areaToJson(area))
    }

    fun broadcastAreaDelete(id: String) {
        broadcastChange("area", id, JSONObject(), deleted = true)
    }

    fun broadcastTagChange(tag: com.kazui.delphi.data.model.Tag) {
        broadcastChange("tag", tag.id, SyncEntityParser.tagToJson(tag))
    }

    fun broadcastTagDelete(id: String) {
        broadcastChange("tag", id, JSONObject(), deleted = true)
    }

    fun broadcastHeadingChange(heading: com.kazui.delphi.data.model.Heading) {
        broadcastChange("heading", heading.id, SyncEntityParser.headingToJson(heading))
    }

    fun broadcastHeadingDelete(id: String) {
        broadcastChange("heading", id, JSONObject(), deleted = true)
    }

    // ---------------------------------------------------------------------------
    // Peer list handling
    // ---------------------------------------------------------------------------

    private suspend fun handlePeerListFromClient(peerList: List<PeerRecord>) {
        for (record in peerList) {
            if (record.deviceId == deviceId) continue
            syncServer.registerExternalPeer(record.deviceId, record.deviceName, record.addresses)

            // Try connecting to newly discovered peers
            if (!syncServer.isConnectedTo(record.deviceId)) {
                scope.launch {
                    connectToPeer(record)
                }
            }
        }
    }

    // ---------------------------------------------------------------------------
    // Helpers
    // ---------------------------------------------------------------------------

    fun getKnownPeers(): List<PeerRecord> = syncServer.getKnownPeerRecords()

    private fun updatePeerCounts() {
        val serverPeers = syncServer.getConnectedPeers()
        val clientState = lanSyncClient.state.value
        val clientConnected = clientState == LanSyncState.LIVE || clientState == LanSyncState.SYNCING || clientState == LanSyncState.CONNECTED
        val totalConnected = serverPeers.size + (if (clientConnected) 1 else 0)

        _connectedPeerCount.value = totalConnected
        _connectedPeerNames.value = buildList {
            addAll(serverPeers.map { it.second })
            if (clientConnected) {
                val serverInfo = lanSyncClient.serverInfo.value
                if (serverInfo != null && serverInfo.deviceName.isNotEmpty()) {
                    add(serverInfo.deviceName)
                }
            }
        }
    }

    /**
     * Extract IP (host) from an address string like "192.168.1.70:21531" or "[fe80::1%en0]:21531"
     * Also handles bare IPv6 without brackets: "fe80::1%en0:21531"
     */
    private fun extractIpFromAddress(addr: String): String? {
        val trimmed = addr.trim()
        if (trimmed.startsWith("[")) {
            // Bracketed IPv6: [fe80::1%en0]:21531
            val closeBracket = trimmed.indexOf(']')
            if (closeBracket < 0) return null
            return trimmed.substring(1, closeBracket)
        }
        // Check if this looks like IPv6 (contains multiple colons)
        val colonCount = trimmed.count { it == ':' }
        if (colonCount > 1) {
            // Bare IPv6 with port: "fe80::1%en0:21531"
            // The port is after the last colon, but only if what follows is a pure number
            val lastColon = trimmed.lastIndexOf(':')
            val afterLastColon = trimmed.substring(lastColon + 1)
            return if (afterLastColon.all { it.isDigit() } && afterLastColon.isNotEmpty()) {
                // Strip the port part
                trimmed.substring(0, lastColon)
            } else {
                // No port, entire string is the IPv6 address
                trimmed
            }
        }
        // IPv4: 192.168.1.70:21531
        val colonIdx = trimmed.lastIndexOf(':')
        return if (colonIdx < 0) trimmed else trimmed.substring(0, colonIdx)
    }
}
