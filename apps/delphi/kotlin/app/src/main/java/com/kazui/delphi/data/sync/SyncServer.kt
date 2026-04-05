package com.kazui.delphi.data.sync

import android.util.Log
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import com.kazui.delphi.data.model.TodoItem
import com.kazui.delphi.data.model.Project
import com.kazui.delphi.di.DatabaseProvider
import io.ktor.server.application.*
import io.ktor.server.cio.*
import io.ktor.server.engine.*
import io.ktor.server.routing.*
import io.ktor.server.websocket.*
import io.ktor.websocket.*
import kotlin.time.Duration.Companion.milliseconds
import kotlin.time.Duration.Companion.seconds
import kotlinx.coroutines.*
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import org.json.JSONArray
import org.json.JSONObject
import java.time.Instant
import java.util.concurrent.ConcurrentHashMap
import javax.inject.Inject
import javax.inject.Singleton

private const val TAG = "SyncServer"
private const val PROTOCOL_VERSION = 1
private const val MAX_BATCH_SIZE = 100
private const val PING_INTERVAL_MS = 15_000L

/**
 * Ktor CIO embedded WebSocket server for equal-peer P2P sync.
 *
 * Protocol flow per incoming connection:
 *   1. Client sends hello (with addresses[]) -> server validates, replies with hello
 *   2. Both exchange version_vector messages
 *   3. Both exchange peer_list messages
 *   4. Both exchange sync_changes batches with ACK
 *   5. Live mode: mutations broadcast as live_change with live_ack
 */
@Singleton
class SyncServer @Inject constructor(
    private val databaseProvider: DatabaseProvider,
    private val dataStore: DataStore<Preferences>,
) {
    private val versionVectorKey = stringPreferencesKey("lan_sync.version_vector")
    private val knownPeersKey = stringPreferencesKey("sync.peers")
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    private var server: EmbeddedServer<CIOApplicationEngine, CIOApplicationEngine.Configuration>? = null
    @Volatile private var isRunning = false

    private var spaceId: String = ""
    private var deviceId: String = ""
    private var deviceName: String = ""
    private var ownAddresses: List<String> = emptyList()

    // Connected peers: session -> PeerState
    private val peers = ConcurrentHashMap<WebSocketServerSession, PeerState>()
    private val vectorMutex = Mutex()

    /** Known peer records persisted across sessions. */
    private var knownPeerRecords = mutableListOf<PeerRecord>()
    private val peerRecordsMutex = Mutex()

    // Callbacks
    var onChangeReceived: ((JSONObject) -> Unit)? = null
    var onPeerConnected: ((String, String) -> Unit)? = null  // deviceId, deviceName
    var onPeerDisconnected: ((String) -> Unit)? = null
    var onNewPeerDiscovered: ((PeerRecord) -> Unit)? = null

    private data class PeerState(
        val session: WebSocketServerSession,
        var deviceId: String = "",
        var deviceName: String = "",
        var addresses: List<String> = emptyList(),
        var authenticated: Boolean = false,
        var syncComplete: Boolean = false,
        val queuedLiveChanges: MutableList<JSONObject> = mutableListOf(),
    )

    val connectedPeerCount: Int
        get() = peers.values.count { it.authenticated }

    fun getConnectedPeers(): List<Pair<String, String>> =
        peers.values.filter { it.authenticated }.map { it.deviceId to it.deviceName }

    fun getKnownPeerRecords(): List<PeerRecord> = knownPeerRecords.toList()

    fun addKnownPeer(record: PeerRecord) {
        val merged = mergePeerRecords(knownPeerRecords, listOf(record))
        knownPeerRecords.clear()
        knownPeerRecords.addAll(merged)
        scope.launch { saveKnownPeers() }
    }

    fun isConnectedTo(deviceId: String): Boolean =
        peers.values.any { it.deviceId == deviceId && it.authenticated }

    // ---------------------------------------------------------------------------
    // Start / Stop
    // ---------------------------------------------------------------------------

    /** The port the server is actually listening on (may differ from requested if port was busy). */
    @Volatile var actualPort: Int = 0
        private set

    suspend fun start(port: Int, spaceId: String, deviceId: String, deviceName: String, ownAddresses: List<String>) {
        if (isRunning) stop()

        this.spaceId = spaceId
        this.deviceId = deviceId
        this.deviceName = deviceName
        this.ownAddresses = ownAddresses

        loadKnownPeers()

        isRunning = true

        // Try the requested port, then fall back to next ports up to +10
        val maxPort = port + 10
        var lastException: Exception? = null
        for (tryPort in port..maxPort) {
            // Pre-check: is the port actually free? Ktor CIO binds async,
            // so a BindException would crash the app on a background dispatcher.
            if (!isPortAvailable(tryPort)) {
                Log.w(TAG, "Port $tryPort busy (pre-check), trying next...")
                continue
            }

            server = embeddedServer(CIO, port = tryPort) {
                install(io.ktor.server.websocket.WebSockets) {
                    pingPeriod = PING_INTERVAL_MS.milliseconds
                    timeout = 30.seconds
                    maxFrameSize = Long.MAX_VALUE
                    masking = false
                }
                routing {
                    webSocket("/") {
                        handleConnection(this)
                    }
                }
            }

            try {
                server?.start(wait = false)
                // Give Ktor CIO a moment to actually bind
                delay(200)
                actualPort = tryPort
                Log.i(TAG, "Server listening on port $tryPort, ${knownPeerRecords.size} known peers")
                return
            } catch (e: java.net.BindException) {
                Log.w(TAG, "Port $tryPort busy, trying next...")
                lastException = e
                server = null
            } catch (e: Exception) {
                Log.e(TAG, "Failed to start server on port $tryPort: ${e.message}")
                lastException = e
                server = null
            }
        }

        // All ports failed — do NOT throw, just log; app can still work as client-only
        isRunning = false
        Log.e(TAG, "Failed to start server on any port in $port..$maxPort (will work as client only)")
    }

    suspend fun stop() {
        isRunning = false
        peers.clear()
        try {
            server?.stop(1000, 2000)
        } catch (e: Exception) {
            Log.w(TAG, "Error stopping server: ${e.message}")
        }
        server = null
        actualPort = 0
        Log.i(TAG, "Server stopped")

        // Wait for OS to release the port
        delay(300)
    }

    /** Check if a port is available before letting Ktor CIO try to bind it async. */
    private fun isPortAvailable(port: Int): Boolean {
        return try {
            java.nio.channels.ServerSocketChannel.open().use { ch ->
                ch.socket().reuseAddress = true
                ch.socket().bind(java.net.InetSocketAddress(port))
            }
            true
        } catch (_: Exception) {
            false
        }
    }

    fun setOwnAddresses(addresses: List<String>) {
        this.ownAddresses = addresses
    }

    /** Register an externally-managed peer connection (from SyncClient). */
    fun registerExternalPeer(deviceId: String, deviceName: String, addresses: List<String>) {
        scope.launch {
            updatePeerRecord(deviceId, deviceName, addresses)
        }
    }

    // ---------------------------------------------------------------------------
    // Connection handling
    // ---------------------------------------------------------------------------

    private suspend fun handleConnection(session: WebSocketServerSession) {
        val peer = PeerState(session = session)
        peers[session] = peer

        try {
            for (frame in session.incoming) {
                if (frame is Frame.Text) {
                    try {
                        val msg = JSONObject(frame.readText())
                        handleMessage(session, peer, msg)
                    } catch (e: Exception) {
                        Log.e(TAG, "Error handling message: ${e.message}")
                    }
                }
            }
        } catch (e: Exception) {
            Log.w(TAG, "Peer connection error: ${e.message}")
        } finally {
            peers.remove(session)
            if (peer.authenticated) {
                Log.i(TAG, "Peer disconnected: ${peer.deviceName} (${peer.deviceId})")
                onPeerDisconnected?.invoke(peer.deviceId)
            }
        }
    }

    private suspend fun handleMessage(session: WebSocketServerSession, peer: PeerState, msg: JSONObject) {
        when (msg.optString("type")) {
            "hello" -> handleHello(session, peer, msg)
            "version_vector" -> handleVersionVector(session, peer, msg)
            "sync_changes" -> handleSyncChanges(session, peer, msg)
            "sync_ack" -> { /* ACK received */ }
            "live_change" -> handleLiveChange(session, peer, msg)
            "live_ack" -> { /* ACK received */ }
            "peer_list" -> handlePeerList(peer, msg)
            "ping" -> {
                val pong = JSONObject().apply {
                    put("type", "pong")
                    put("ts", msg.optLong("ts"))
                }
                safeSend(session, pong.toString())
            }
            "pong" -> { /* heartbeat */ }
        }
    }

    /** Send text on a WebSocketServerSession, catching ClosedSendChannelException gracefully. */
    private suspend fun safeSend(session: WebSocketServerSession, text: String): Boolean {
        return try {
            session.send(text)
            true
        } catch (e: Exception) {
            if (e is kotlinx.coroutines.CancellationException) throw e
            Log.w(TAG, "safeSend failed (session closed?): ${e.message}")
            false
        }
    }

    // ---------------------------------------------------------------------------
    // Handshake
    // ---------------------------------------------------------------------------

    private suspend fun handleHello(session: WebSocketServerSession, peer: PeerState, msg: JSONObject) {
        val version = msg.optInt("protocol_version", 0)
        if (version != PROTOCOL_VERSION) {
            Log.w(TAG, "Protocol version mismatch: $version vs $PROTOCOL_VERSION")
            session.close(CloseReason(CloseReason.Codes.PROTOCOL_ERROR, "protocol version mismatch"))
            return
        }

        peer.deviceId = msg.optString("device_id", "")
        peer.deviceName = msg.optString("device_name", "")
        peer.addresses = jsonArrayToStringList(msg.optJSONArray("addresses"))
        peer.authenticated = true

        Log.i(TAG, "Peer authenticated: ${peer.deviceName}")

        // Merge announced addresses into known peer records
        updatePeerRecord(peer.deviceId, peer.deviceName, peer.addresses)

        // Send our hello back
        val hello = JSONObject().apply {
            put("type", "hello")
            put("protocol_version", PROTOCOL_VERSION)
            put("device_id", deviceId)
            put("device_name", deviceName)
            put("space_id", spaceId)
            put("addresses", JSONArray(ownAddresses))
        }
        if (!safeSend(session, hello.toString())) return

        onPeerConnected?.invoke(peer.deviceId, peer.deviceName)

        // Send version vector
        sendVersionVector(session)

        // Send peer list after a short delay
        scope.launch {
            try {
                delay(100)
                sendPeerList(session)
            } catch (e: Exception) {
                if (e is kotlinx.coroutines.CancellationException) throw e
                Log.w(TAG, "Failed to send peer list: ${e.message}")
            }
        }
    }

    // ---------------------------------------------------------------------------
    // Peer list exchange
    // ---------------------------------------------------------------------------

    private suspend fun sendPeerList(session: WebSocketServerSession) {
        val peersJson = JSONArray()
        for (record in knownPeerRecords) {
            peersJson.put(record.toJson())
        }
        val msg = JSONObject().apply {
            put("type", "peer_list")
            put("peers", peersJson)
        }
        safeSend(session, msg.toString())
    }

    private suspend fun handlePeerList(peer: PeerState, msg: JSONObject) {
        if (!peer.authenticated) return

        val incomingPeers = mutableListOf<PeerRecord>()
        val peersArray = msg.optJSONArray("peers") ?: return
        for (i in 0 until peersArray.length()) {
            val peerJson = peersArray.optJSONObject(i) ?: continue
            val record = PeerRecord.fromJson(peerJson)
            if (record.deviceId != deviceId) {
                incomingPeers.add(record)
            }
        }

        val beforeCount = knownPeerRecords.size
        peerRecordsMutex.withLock {
            knownPeerRecords = mergePeerRecords(knownPeerRecords, incomingPeers).toMutableList()
        }
        saveKnownPeers()

        // Notify about newly discovered peers we should connect to
        for (incoming in incomingPeers) {
            if (incoming.deviceId == peer.deviceId) continue
            if (isConnectedTo(incoming.deviceId)) continue
            val record = knownPeerRecords.find { it.deviceId == incoming.deviceId }
            if (record != null) {
                onNewPeerDiscovered?.invoke(record)
            }
        }

        if (knownPeerRecords.size > beforeCount) {
            Log.i(TAG, "Peer list updated: $beforeCount -> ${knownPeerRecords.size} known peers")
        }
    }

    // ---------------------------------------------------------------------------
    // Version vector exchange
    // ---------------------------------------------------------------------------

    private suspend fun sendVersionVector(session: WebSocketServerSession) {
        var vector = loadVersionVector()

        if (vector.length() == 0) {
            loadAllEntities(vector)
            vector = loadVersionVector()
        }

        val msg = JSONObject().apply {
            put("type", "version_vector")
            put("vector", vector)
        }
        safeSend(session, msg.toString())
    }

    private suspend fun handleVersionVector(session: WebSocketServerSession, peer: PeerState, msg: JSONObject) {
        if (!peer.authenticated) return

        var localVector = loadVersionVector()
        val remoteVector = msg.optJSONObject("vector") ?: JSONObject()

        if (localVector.length() == 0) {
            val allEntities = loadAllEntities(localVector)
            localVector = loadVersionVector()

            if (allEntities.isNotEmpty()) {
                sendBatches(session, allEntities)
            } else {
                val emptyBatch = JSONObject().apply {
                    put("type", "sync_changes")
                    put("batch_id", generateId())
                    put("entities", JSONArray())
                    put("is_last", true)
                }
                safeSend(session, emptyBatch.toString())
            }

            peer.syncComplete = true
            Log.i(TAG, "Sync complete with ${peer.deviceName} (sent our batches, first connect)")
            flushQueuedLiveChanges(session, peer)
            return
        }

        val allEntities = loadAllEntities(localVector)
        val toSend = mutableListOf<JSONObject>()

        for (entity in allEntities) {
            val entityId = entity.optString("id")
            val entityHlc = entity.optString("hlc")
            val remoteHlc = remoteVector.optString(entityId, "")
            if (remoteHlc.isEmpty() || isNewerHlc(entityHlc, remoteHlc)) {
                toSend.add(entity)
            }
        }

        Log.i(TAG, "Version vector diff: ${toSend.size}/${allEntities.size} entities to send to ${peer.deviceName}")

        if (toSend.isNotEmpty()) {
            sendBatches(session, toSend)
        } else {
            val emptyBatch = JSONObject().apply {
                put("type", "sync_changes")
                put("batch_id", generateId())
                put("entities", JSONArray())
                put("is_last", true)
            }
            safeSend(session, emptyBatch.toString())
        }

        peer.syncComplete = true
        Log.i(TAG, "Sync complete with ${peer.deviceName} (sent ${toSend.size} entities)")
        flushQueuedLiveChanges(session, peer)
    }

    // ---------------------------------------------------------------------------
    // Sync batches
    // ---------------------------------------------------------------------------

    private suspend fun sendBatches(session: WebSocketServerSession, entities: List<JSONObject>) {
        val batches = entities.chunked(MAX_BATCH_SIZE)
        if (batches.isEmpty()) {
            val emptyBatch = JSONObject().apply {
                put("type", "sync_changes")
                put("batch_id", generateId())
                put("entities", JSONArray())
                put("is_last", true)
            }
            safeSend(session, emptyBatch.toString())
            return
        }

        batches.forEachIndexed { i, batch ->
            val batchMsg = JSONObject().apply {
                put("type", "sync_changes")
                put("batch_id", generateId())
                put("entities", JSONArray(batch.map { it.toString() }.map { JSONObject(it) }))
                put("is_last", i == batches.size - 1)
            }
            if (!safeSend(session, batchMsg.toString())) return
        }
    }

    private suspend fun handleSyncChanges(session: WebSocketServerSession, peer: PeerState, msg: JSONObject) {
        if (!peer.authenticated) return

        val batchId = msg.optString("batch_id", "")
        val entities = msg.optJSONArray("entities") ?: JSONArray()
        val isLast = msg.optBoolean("is_last", false)
        var accepted = 0

        val localVector = loadVersionVector()

        for (i in 0 until entities.length()) {
            val entity = entities.optJSONObject(i) ?: continue
            val entityId = entity.optString("id", "")
            val hlc = entity.optString("hlc", "")
            val localHlc = localVector.optString(entityId, "")

            if (localHlc.isEmpty() || isNewerHlc(hlc, localHlc)) {
                if (applySyncEntity(entity)) {
                    localVector.put(entityId, hlc)
                    accepted++
                    onChangeReceived?.invoke(entity)
                    broadcastLiveChange(entity, peer.deviceId)
                }
            }
        }

        if (accepted > 0) {
            saveVersionVector(localVector)
        }

        val ack = JSONObject().apply {
            put("type", "sync_ack")
            put("batch_id", batchId)
            put("accepted", accepted)
        }
        safeSend(session, ack.toString())

        if (isLast) {
            Log.i(TAG, "Received all sync batches from ${peer.deviceName} (accepted=$accepted)")
        }
    }

    // ---------------------------------------------------------------------------
    // Live mode
    // ---------------------------------------------------------------------------

    /** Broadcast a live change to all authenticated peers. */
    fun broadcastLiveChange(entity: JSONObject, excludeDeviceId: String? = null) {
        val changeId = generateId()
        val msg = JSONObject().apply {
            put("type", "live_change")
            put("change_id", changeId)
            put("entity", entity)
        }

        for ((session, peer) in peers) {
            if (!peer.authenticated) continue
            if (peer.deviceId == excludeDeviceId) continue

            if (!peer.syncComplete) {
                peer.queuedLiveChanges.add(entity)
                continue
            }

            scope.launch {
                try {
                    session.send(msg.toString())
                } catch (e: Exception) {
                    Log.w(TAG, "Failed to send live change to ${peer.deviceName}: ${e.message}")
                }
            }
        }
    }

    private suspend fun flushQueuedLiveChanges(session: WebSocketServerSession, peer: PeerState) {
        if (peer.queuedLiveChanges.isEmpty()) return

        Log.i(TAG, "Flushing ${peer.queuedLiveChanges.size} queued live changes to ${peer.deviceName}")
        for (entity in peer.queuedLiveChanges) {
            val msg = JSONObject().apply {
                put("type", "live_change")
                put("change_id", generateId())
                put("entity", entity)
            }
            if (!safeSend(session, msg.toString())) break
        }
        peer.queuedLiveChanges.clear()
    }

    private suspend fun handleLiveChange(session: WebSocketServerSession, peer: PeerState, msg: JSONObject) {
        if (!peer.authenticated) return

        val changeId = msg.optString("change_id", "")
        val entity = msg.optJSONObject("entity") ?: return
        val entityId = entity.optString("id", "")
        val hlc = entity.optString("hlc", "")

        val localVector = loadVersionVector()
        val localHlc = localVector.optString(entityId, "")

        if (localHlc.isEmpty() || isNewerHlc(hlc, localHlc)) {
            if (applySyncEntity(entity)) {
                localVector.put(entityId, hlc)
                saveVersionVector(localVector)
                onChangeReceived?.invoke(entity)
                broadcastLiveChange(entity, peer.deviceId)
            }
        }

        val ack = JSONObject().apply {
            put("type", "live_ack")
            put("change_id", changeId)
        }
        safeSend(session, ack.toString())
    }

    // ---------------------------------------------------------------------------
    // Entity persistence
    // ---------------------------------------------------------------------------

    private suspend fun applySyncEntity(entityJson: JSONObject): Boolean {
        try {
            val entityType = entityJson.optString("type", "")
            val entityId = entityJson.optString("id", "")
            val deleted = entityJson.optBoolean("deleted", false)
            val data = entityJson.optJSONObject("data")

            if (entityId.isEmpty()) return false

            val repo = databaseProvider.arkDataRepository
            if (deleted) {
                when (entityType) {
                    "todo" -> repo.deleteById(entityId)
                    "project" -> repo.deleteProjectById(entityId)
                    "heading" -> repo.deleteHeadingById(entityId)
                }
                return true
            }

            if (data == null) return false

            when (entityType) {
                "todo" -> {
                    val todo = SyncEntityParser.jsonToTodoItem(data, entityId) ?: return false
                    repo.upsert(todo)
                    return true
                }
                "project" -> {
                    val project = SyncEntityParser.jsonToProject(data, entityId) ?: return false
                    repo.upsertProject(project)
                    return true
                }
                "area" -> {
                    val area = SyncEntityParser.jsonToArea(data, entityId) ?: return false
                    repo.upsertArea(area)
                    return true
                }
                "tag" -> {
                    val tag = SyncEntityParser.jsonToTag(data, entityId) ?: return false
                    repo.upsertTag(tag)
                    return true
                }
                "heading" -> {
                    val heading = SyncEntityParser.jsonToHeading(data, entityId) ?: return false
                    repo.upsertHeading(heading)
                    return true
                }
            }
        } catch (e: Exception) {
            Log.e(TAG, "Failed to apply sync entity: ${e.message}")
        }
        return false
    }

    // ---------------------------------------------------------------------------
    // Load all entities
    // ---------------------------------------------------------------------------

    private suspend fun loadAllEntities(vector: JSONObject): List<JSONObject> {
        val entities = mutableListOf<JSONObject>()
        val repo = databaseProvider.arkDataRepository
        val todos = repo.getAllForSync()
        val projects = repo.getAllProjectsForSync()

        for (todo in todos) {
            val hlc = if (vector.has(todo.id)) vector.getString(todo.id) else generateHlc()
            entities.add(JSONObject().apply {
                put("type", "todo")
                put("id", todo.id)
                put("data", SyncEntityParser.todoToJson(todo))
                put("hlc", hlc)
            })
            if (!vector.has(todo.id)) vector.put(todo.id, hlc)
        }

        for (project in projects) {
            val hlc = if (vector.has(project.id)) vector.getString(project.id) else generateHlc()
            entities.add(JSONObject().apply {
                put("type", "project")
                put("id", project.id)
                put("data", SyncEntityParser.projectToJson(project))
                put("hlc", hlc)
            })
            if (!vector.has(project.id)) vector.put(project.id, hlc)
        }

        saveVersionVector(vector)
        return entities
    }

    // ---------------------------------------------------------------------------
    // Peer record management
    // ---------------------------------------------------------------------------

    private suspend fun updatePeerRecord(deviceId: String, deviceName: String, addresses: List<String>) {
        val newRecord = PeerRecord(
            deviceId = deviceId,
            deviceName = deviceName,
            addresses = addresses,
            lastSeen = Instant.now().toString(),
        )
        peerRecordsMutex.withLock {
            knownPeerRecords = mergePeerRecords(knownPeerRecords, listOf(newRecord)).toMutableList()
        }
        saveKnownPeers()
    }

    // ---------------------------------------------------------------------------
    // Version vector persistence (DataStore)
    // ---------------------------------------------------------------------------

    private suspend fun loadVersionVector(): JSONObject {
        return try {
            val prefs = dataStore.data.first()
            val raw = prefs[versionVectorKey] ?: return JSONObject()
            JSONObject(raw)
        } catch (_: Exception) {
            JSONObject()
        }
    }

    private suspend fun saveVersionVector(vector: JSONObject) {
        vectorMutex.withLock {
            dataStore.edit { prefs ->
                prefs[versionVectorKey] = vector.toString()
            }
        }
    }

    // ---------------------------------------------------------------------------
    // Known peers persistence (DataStore)
    // ---------------------------------------------------------------------------

    private suspend fun loadKnownPeers() {
        try {
            val prefs = dataStore.data.first()
            val raw = prefs[knownPeersKey] ?: return
            val parsed = JSONArray(raw)
            val records = mutableListOf<PeerRecord>()
            for (i in 0 until parsed.length()) {
                records.add(PeerRecord.fromJson(parsed.getJSONObject(i)))
            }
            knownPeerRecords = records
        } catch (_: Exception) {
            knownPeerRecords = mutableListOf()
        }
    }

    private suspend fun saveKnownPeers() {
        try {
            val arr = JSONArray()
            for (record in knownPeerRecords) {
                arr.put(record.toJson())
            }
            dataStore.edit { prefs ->
                prefs[knownPeersKey] = arr.toString()
            }
        } catch (e: Exception) {
            Log.w(TAG, "Failed to save known peers: ${e.message}")
        }
    }

    /**
     * Update the version vector for a locally-mutated entity.
     * Called by PeerManager when the user makes a local change.
     */
    suspend fun updateEntityHlc(entityId: String): String {
        val vector = loadVersionVector()
        val hlcStr = generateHlc()
        vector.put(entityId, hlcStr)
        saveVersionVector(vector)
        return hlcStr
    }

    // ---------------------------------------------------------------------------
    // HLC utilities
    // ---------------------------------------------------------------------------

    private fun generateHlc(): String {
        val now = Instant.now().toString()
        return "$now:000000:$deviceId"
    }

    private fun generateId(): String {
        return "${System.currentTimeMillis()}-${(Math.random() * 1000000).toLong()}"
    }

    private fun isNewerHlc(a: String, b: String): Boolean {
        return compareHlcStrings(a, b) > 0
    }

    private fun compareHlcStrings(a: String, b: String): Int {
        val partsA = splitHlc(a)
        val partsB = splitHlc(b)
        val timeCmp = partsA.first.compareTo(partsB.first)
        if (timeCmp != 0) return timeCmp
        val counterCmp = partsA.second.compareTo(partsB.second)
        if (counterCmp != 0) return counterCmp
        return partsA.third.compareTo(partsB.third)
    }

    private fun splitHlc(hlc: String): Triple<String, Int, String> {
        val zIndex = hlc.indexOf('Z')
        if (zIndex < 0) return Triple(hlc, 0, "")
        val rest = hlc.substring(zIndex + 2)
        val colonIndex = rest.indexOf(':')
        if (colonIndex < 0) return Triple(hlc.substring(0, zIndex + 1), 0, rest)
        val counter = rest.substring(0, colonIndex).toIntOrNull() ?: 0
        val deviceId = rest.substring(colonIndex + 1)
        return Triple(hlc.substring(0, zIndex + 1), counter, deviceId)
    }

    private fun jsonArrayToStringList(arr: JSONArray?): List<String> {
        if (arr == null) return emptyList()
        return (0 until arr.length()).map { arr.getString(it) }
    }
}
