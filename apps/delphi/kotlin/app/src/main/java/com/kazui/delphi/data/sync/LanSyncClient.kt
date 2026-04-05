package com.kazui.delphi.data.sync

import android.util.Log
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import com.kazui.delphi.data.model.TodoItem
import com.kazui.delphi.data.model.Project
import com.kazui.delphi.di.DatabaseProvider
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
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import org.json.JSONArray
import org.json.JSONObject
import java.time.Instant
import java.util.concurrent.TimeUnit
import java.util.concurrent.ConcurrentHashMap
import javax.inject.Inject
import javax.inject.Singleton

private const val TAG = "LanSyncClient"
private const val LAN_SYNC_PORT = 21531
private const val PROTOCOL_VERSION = 1
private const val MAX_BATCH_SIZE = 100
private const val RECONNECT_DELAY_MS = 5_000L
private const val MAX_RETRIES = 3

enum class LanSyncState {
    DISCONNECTED,
    CONNECTING,
    CONNECTED,
    SYNCING,
    LIVE,
}

@Singleton
class LanSyncClient @Inject constructor(
    private val databaseProvider: DatabaseProvider,
    private val dataStore: DataStore<Preferences>,
) {
    private val versionVectorKey = stringPreferencesKey("lan_sync.version_vector")
    private val serverInfoKey = stringPreferencesKey("lan_sync.server_info")
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    private val okHttpClient = OkHttpClient.Builder()
        .readTimeout(0, TimeUnit.MILLISECONDS)  // no read timeout for WS
        .connectTimeout(10, TimeUnit.SECONDS)
        .build()

    private var webSocket: WebSocket? = null
    private var reconnectJob: Job? = null
    private var syncJob: Job? = null  // tracks coroutines from current connection
    @Volatile private var isRunning = false
    @Volatile private var connectionGeneration = 0  // incremented on each connect to invalidate old callbacks

    private var serverIp: String = ""
    private var spaceId: String = ""
    private var deviceId: String = ""
    private var deviceName: String = ""
    private var ownAddresses: List<String> = emptyList()

    // Version vector: entity_id -> HLC string
    private val versionVector = ConcurrentHashMap<String, String>()
    @Volatile private var vectorBuilt = false

    private val _state = MutableStateFlow(LanSyncState.DISCONNECTED)
    val state: StateFlow<LanSyncState> = _state.asStateFlow()

    /** Server info (device_name, last_seen) persisted across sessions. */
    data class ServerInfo(val deviceName: String, val lastSeen: String)
    private val _serverInfo = MutableStateFlow<ServerInfo?>(null)
    val serverInfo: StateFlow<ServerInfo?> = _serverInfo.asStateFlow()

    // Callback for UI updates when data changes from sync
    var onDataChanged: (() -> Unit)? = null

    /** Callback when peer_list message is received from server. */
    var onPeerListReceived: ((List<PeerRecord>) -> Unit)? = null

    init {
        // Load persisted server info on creation
        scope.launch {
            try {
                val prefs = dataStore.data.first()
                val raw = prefs[serverInfoKey]
                if (raw != null) {
                    val json = JSONObject(raw)
                    _serverInfo.value = ServerInfo(
                        deviceName = json.optString("device_name", ""),
                        lastSeen = json.optString("last_seen", ""),
                    )
                }
            } catch (e: Exception) {
                Log.w(TAG, "Failed to load server info: ${e.message}")
            }
        }
    }

    /**
     * Connect to a peer at the given IP.
     * @param ownAddrs own addresses to announce in hello
     */
    fun connect(ip: String, spaceId: String, deviceId: String, deviceName: String, ownAddrs: List<String> = emptyList()) {
        this.serverIp = ip.trim()
        this.spaceId = spaceId
        this.deviceId = deviceId
        this.deviceName = deviceName
        this.ownAddresses = ownAddrs
        isRunning = true
        _state.value = LanSyncState.CONNECTING
        doConnect()
    }

    fun disconnect() {
        isRunning = false
        connectionGeneration++
        reconnectJob?.cancel()
        reconnectJob = null
        syncJob?.cancel()
        syncJob = null
        webSocket?.close(1000, "client disconnecting")
        webSocket = null
        vectorBuilt = false
        _state.value = LanSyncState.DISCONNECTED
        Log.i(TAG, "Disconnected")
    }

    /** Clear version vector and in-memory state. Called on space change or data clear only. */
    fun clearSyncState() {
        versionVector.clear()
        vectorBuilt = false
        scope.launch {
            dataStore.edit { prefs -> prefs.remove(versionVectorKey) }
        }
        Log.i(TAG, "Sync state cleared")
    }

    /** Send a local change to the server in live mode. */
    fun sendLiveChange(entityType: String, entityId: String, data: JSONObject, deleted: Boolean = false) {
        val ws = webSocket ?: return
        if (_state.value != LanSyncState.LIVE) return

        val hlc = generateHlc()
        versionVector[entityId] = hlc
        scope.launch { persistVersionVector() }

        val entity = JSONObject().apply {
            put("type", entityType)
            put("id", entityId)
            put("data", data)
            put("hlc", hlc)
            if (deleted) put("deleted", true)
        }

        val msg = JSONObject().apply {
            put("type", "live_change")
            put("change_id", generateId())
            put("entity", entity)
        }

        ws.send(msg.toString())
    }

    // -------------------------------------------------------------------------
    // High-level broadcast helpers (called from ViewModels after DAO mutations)
    // -------------------------------------------------------------------------

    fun broadcastTodoChange(todo: TodoItem) {
        sendLiveChange("todo", todo.id, SyncEntityParser.todoToJson(todo))
    }

    fun broadcastTodoDelete(id: String) {
        sendLiveChange("todo", id, JSONObject(), deleted = true)
    }

    fun broadcastProjectChange(project: Project) {
        sendLiveChange("project", project.id, SyncEntityParser.projectToJson(project))
    }

    fun broadcastProjectDelete(id: String) {
        sendLiveChange("project", id, JSONObject(), deleted = true)
    }

    // -------------------------------------------------------------------------
    // Internal
    // -------------------------------------------------------------------------

    private fun doConnect() {
        if (!isRunning) return

        // Validate IP before creating URL
        if (serverIp.isBlank()) {
            Log.w(TAG, "doConnect: empty server IP, skipping")
            return
        }

        // Cancel any ongoing work from previous connection
        syncJob?.cancel()
        syncJob = null
        vectorBuilt = false

        // Close previous WebSocket if still lingering
        webSocket?.let {
            try { it.cancel() } catch (_: Exception) {}
            webSocket = null
        }

        connectionGeneration++
        val thisGeneration = connectionGeneration

        // IPv6 addresses must be wrapped in brackets for URLs; strip zone ID (e.g. %en0) which OkHttp rejects
        val cleanIp = serverIp.substringBefore('%')
        val host = if (cleanIp.contains(':')) "[$cleanIp]" else cleanIp
        val url = "ws://$host:$LAN_SYNC_PORT"
        Log.i(TAG, "Connecting to $url (gen=$thisGeneration)")

        val request = try {
            Request.Builder().url(url).build()
        } catch (e: IllegalArgumentException) {
            Log.e(TAG, "Invalid URL '$url': ${e.message}")
            scheduleReconnect()
            return
        }

        okHttpClient.newWebSocket(request, object : WebSocketListener() {
            override fun onOpen(ws: WebSocket, response: Response) {
                if (thisGeneration != connectionGeneration) {
                    Log.w(TAG, "Stale onOpen (gen=$thisGeneration, current=$connectionGeneration), closing")
                    ws.cancel()
                    return
                }
                webSocket = ws
                _state.value = LanSyncState.CONNECTED
                Log.i(TAG, "Connected to server (gen=$thisGeneration)")

                // Send hello with own addresses
                val hello = JSONObject().apply {
                    put("type", "hello")
                    put("protocol_version", PROTOCOL_VERSION)
                    put("device_id", deviceId)
                    put("device_name", deviceName)
                    put("space_id", spaceId)
                    put("addresses", JSONArray(ownAddresses))
                }
                ws.send(hello.toString())
            }

            override fun onMessage(ws: WebSocket, text: String) {
                if (thisGeneration != connectionGeneration) return
                try {
                    val msg = JSONObject(text)
                    handleMessage(ws, msg)
                } catch (e: Exception) {
                    Log.e(TAG, "Error handling message: ${e.message}", e)
                }
            }

            override fun onClosed(ws: WebSocket, code: Int, reason: String) {
                if (thisGeneration != connectionGeneration) return
                webSocket = null
                _state.value = LanSyncState.DISCONNECTED
                Log.i(TAG, "Connection closed: code=$code reason=$reason (gen=$thisGeneration)")
                scheduleReconnect()
            }

            override fun onFailure(ws: WebSocket, t: Throwable, response: Response?) {
                if (thisGeneration != connectionGeneration) {
                    Log.d(TAG, "Stale onFailure ignored (gen=$thisGeneration)")
                    return
                }
                ws.cancel()  // OkHttp requires cancel() in onFailure
                webSocket = null
                _state.value = LanSyncState.DISCONNECTED
                Log.w(TAG, "Connection failed (gen=$thisGeneration): ${t.message}")
                scheduleReconnect()
            }
        })
    }

    private fun handleMessage(ws: WebSocket, msg: JSONObject) {
        when (msg.optString("type")) {
            "hello" -> handleHello(ws, msg)
            "version_vector" -> handleVersionVector(ws, msg)
            "sync_changes" -> handleSyncChanges(ws, msg)
            "sync_ack" -> { /* ACK for our batches */ }
            "live_change" -> handleLiveChange(ws, msg)
            "live_ack" -> { /* ACK received */ }
            "peer_list" -> handlePeerListMessage(msg)
            "ping" -> {
                val pong = JSONObject().apply {
                    put("type", "pong")
                    put("ts", msg.optLong("ts"))
                }
                ws.send(pong.toString())
            }
            "pong" -> { /* heartbeat response */ }
        }
    }

    private fun handleHello(ws: WebSocket, msg: JSONObject) {
        val version = msg.optInt("protocol_version", 0)
        if (version != PROTOCOL_VERSION) {
            Log.w(TAG, "Protocol version mismatch: $version vs $PROTOCOL_VERSION")
            ws.close(1002, "protocol version mismatch")
            return
        }

        val serverDeviceName = msg.optString("device_name", "")
        val serverDeviceId = msg.optString("device_id", "")
        val serverAddresses = jsonArrayToStringList(msg.optJSONArray("addresses"))
        val gen = connectionGeneration

        Log.i(TAG, "Server hello: $serverDeviceName (id=$serverDeviceId, gen=$gen)")
        _state.value = LanSyncState.SYNCING

        // Persist server info
        scope.launch {
            val info = ServerInfo(serverDeviceName, Instant.now().toString())
            _serverInfo.value = info
            try {
                dataStore.edit { prefs ->
                    prefs[serverInfoKey] = JSONObject().apply {
                        put("device_name", info.deviceName)
                        put("last_seen", info.lastSeen)
                    }.toString()
                }
            } catch (_: Exception) {}
        }

        // Build and send our version vector — tracked in syncJob for cancellation on reconnect
        syncJob?.cancel()
        syncJob = scope.launch {
            try {
                buildVersionVector()
                vectorBuilt = true
                val vectorJson = JSONObject()
                versionVector.forEach { (k, v) -> vectorJson.put(k, v) }

                if (gen != connectionGeneration) return@launch
                val vectorMsg = JSONObject().apply {
                    put("type", "version_vector")
                    put("vector", vectorJson)
                }
                if (!ws.send(vectorMsg.toString())) {
                    Log.w(TAG, "Failed to send version_vector (ws closed, gen=$gen)")
                    return@launch
                }

                // Send our peer list after version vector
                delay(100)
                if (gen != connectionGeneration) return@launch
                sendPeerList(ws)
            } catch (e: Exception) {
                if (e is kotlinx.coroutines.CancellationException) throw e
                Log.e(TAG, "Error in handleHello sync job: ${e.message}", e)
            }
        }
    }

    /** Send our known peer list to the server. */
    private fun sendPeerList(ws: WebSocket) {
        // We send an empty peer list since PeerManager manages the full list via SyncServer.
        // The server will send its peer list to us.
        val msg = JSONObject().apply {
            put("type", "peer_list")
            put("peers", JSONArray())
        }
        ws.send(msg.toString())
    }

    /** Handle incoming peer_list from server. */
    private fun handlePeerListMessage(msg: JSONObject) {
        val peersArray = msg.optJSONArray("peers") ?: return
        val peerList = mutableListOf<PeerRecord>()
        for (i in 0 until peersArray.length()) {
            val peerJson = peersArray.optJSONObject(i) ?: continue
            peerList.add(PeerRecord.fromJson(peerJson))
        }

        onPeerListReceived?.invoke(peerList)
    }

    private fun handleVersionVector(ws: WebSocket, msg: JSONObject) {
        val remoteVector = msg.optJSONObject("vector") ?: return
        val gen = connectionGeneration

        scope.launch {
            try {
                // Ensure version vector is built before processing
                if (!vectorBuilt) {
                    buildVersionVector()
                    vectorBuilt = true
                }

                if (gen != connectionGeneration) return@launch

                // Compute what the server needs from us
                val entitiesToSend = mutableListOf<JSONObject>()

                versionVector.forEach { (entityId, localHlc) ->
                    val remoteHlc = remoteVector.optString(entityId, "")
                    if (remoteHlc.isEmpty() || compareHlc(localHlc, remoteHlc) > 0) {
                        val entity = loadEntityById(entityId)
                        if (entity != null) {
                            entitiesToSend.add(entity)
                        }
                    }
                }

                Log.i(TAG, "Version vector diff: ${entitiesToSend.size} entities to send (gen=$gen)")

                if (gen != connectionGeneration) return@launch

                // Send in batches
                if (entitiesToSend.isEmpty()) {
                    val emptyBatch = JSONObject().apply {
                        put("type", "sync_changes")
                        put("batch_id", generateId())
                        put("entities", JSONArray())
                        put("is_last", true)
                    }
                    ws.send(emptyBatch.toString())
                } else {
                    val batches = entitiesToSend.chunked(MAX_BATCH_SIZE)
                    batches.forEachIndexed { i, batch ->
                        if (gen != connectionGeneration) return@launch
                        val batchMsg = JSONObject().apply {
                            put("type", "sync_changes")
                            put("batch_id", generateId())
                            put("entities", JSONArray(batch.map { it.toString() }.map { JSONObject(it) }))
                            put("is_last", i == batches.size - 1)
                        }
                        ws.send(batchMsg.toString())
                    }
                }
            } catch (e: Exception) {
                if (e is kotlinx.coroutines.CancellationException) throw e
                Log.e(TAG, "Error in handleVersionVector: ${e.message}", e)
            }
        }
    }

    private fun handleSyncChanges(ws: WebSocket, msg: JSONObject) {
        val batchId = msg.optString("batch_id", "")
        val entities = msg.optJSONArray("entities") ?: JSONArray()
        val isLast = msg.optBoolean("is_last", false)
        var accepted = 0
        val gen = connectionGeneration

        scope.launch {
            try {
                for (i in 0 until entities.length()) {
                    val entity = entities.optJSONObject(i) ?: continue
                    if (applySyncEntity(entity)) {
                        accepted++
                    }
                }

                // Persist version vector after batch processing
                if (accepted > 0) {
                    persistVersionVector()
                }

                // Send ACK (check generation to avoid sending on stale connection)
                if (gen == connectionGeneration) {
                    val ack = JSONObject().apply {
                        put("type", "sync_ack")
                        put("batch_id", batchId)
                        put("accepted", accepted)
                    }
                    ws.send(ack.toString())
                }

                if (accepted > 0) {
                    onDataChanged?.invoke()
                }

                if (isLast) {
                    if (gen == connectionGeneration) {
                        _state.value = LanSyncState.LIVE
                    }
                    Log.i(TAG, "Initial sync complete, entering live mode (accepted=$accepted, gen=$gen)")
                }
            } catch (e: Exception) {
                if (e is kotlinx.coroutines.CancellationException) throw e
                Log.e(TAG, "Error in handleSyncChanges: ${e.message}", e)
            }
        }
    }

    private fun handleLiveChange(ws: WebSocket, msg: JSONObject) {
        val changeId = msg.optString("change_id", "")
        val entity = msg.optJSONObject("entity") ?: return
        val gen = connectionGeneration
        scope.launch {
            try {
                val applied = applySyncEntity(entity)

                if (applied) {
                    persistVersionVector()
                }

                // Always ACK (if still same connection)
                if (gen == connectionGeneration) {
                    val ack = JSONObject().apply {
                        put("type", "live_ack")
                        put("change_id", changeId)
                    }
                    ws.send(ack.toString())
                }

                if (applied) {
                    onDataChanged?.invoke()
                }
            } catch (e: Exception) {
                if (e is kotlinx.coroutines.CancellationException) throw e
                Log.e(TAG, "Error in handleLiveChange: ${e.message}", e)
            }
        }
    }

    // -------------------------------------------------------------------------
    // Entity handling
    // -------------------------------------------------------------------------

    private suspend fun applySyncEntity(entityJson: JSONObject): Boolean {
        try {
            val entityType = entityJson.optString("type", "")
            val entityId = entityJson.optString("id", "")
            val hlc = entityJson.optString("hlc", "")
            val deleted = entityJson.optBoolean("deleted", false)
            val data = entityJson.optJSONObject("data")
    

            if (entityId.isEmpty()) return false

            // Check if remote is newer
            val localHlc = versionVector[entityId]
            if (localHlc != null && hlc.isNotEmpty() && compareHlc(hlc, localHlc) <= 0) {
                return false // Our version is the same or newer
            }

            val repo = databaseProvider.arkDataRepository
            if (deleted) {
                when (entityType) {
                    "todo" -> repo.deleteById(entityId)
                    "project" -> repo.deleteProjectById(entityId)
                    "heading" -> repo.deleteHeadingById(entityId)
                }
                versionVector[entityId] = hlc.ifEmpty { generateHlc() }
                return true
            }

            if (data == null) return false

            when (entityType) {
                "todo" -> {
                    val todo = SyncEntityParser.jsonToTodoItem(data, entityId) ?: return false
                    repo.upsert(todo)
                    versionVector[entityId] = hlc.ifEmpty { generateHlc() }
                    return true
                }
                "project" -> {
                    val project = SyncEntityParser.jsonToProject(data, entityId) ?: return false
                    repo.upsertProject(project)
                    versionVector[entityId] = hlc.ifEmpty { generateHlc() }
                    return true
                }
                "area" -> {
                    val area = SyncEntityParser.jsonToArea(data, entityId) ?: return false
                    repo.upsertArea(area)
                    versionVector[entityId] = hlc.ifEmpty { generateHlc() }
                    return true
                }
                "tag" -> {
                    val tag = SyncEntityParser.jsonToTag(data, entityId) ?: return false
                    repo.upsertTag(tag)
                    versionVector[entityId] = hlc.ifEmpty { generateHlc() }
                    return true
                }
                "heading" -> {
                    val heading = SyncEntityParser.jsonToHeading(data, entityId) ?: return false
                    repo.upsertHeading(heading)
                    versionVector[entityId] = hlc.ifEmpty { generateHlc() }
                    return true
                }
            }
        } catch (e: Exception) {
            Log.e(TAG, "Failed to apply sync entity: ${e.message}")
        }
        return false
    }

    private suspend fun buildVersionVector() {
        if (versionVector.isEmpty()) {
            loadPersistedVersionVector()
        }

        val repo = databaseProvider.arkDataRepository
        val todos = repo.getAllForSync()
        val projects = repo.getAllProjectsForSync()

        var updated = false
        todos.forEach { todo ->
            if (!versionVector.containsKey(todo.id)) {
                versionVector[todo.id] = generateHlc()
                updated = true
            }
        }
        projects.forEach { project ->
            if (!versionVector.containsKey(project.id)) {
                versionVector[project.id] = generateHlc()
                updated = true
            }
        }

        if (updated) {
            persistVersionVector()
        }
    }

    private suspend fun loadPersistedVersionVector() {
        try {
            val prefs = dataStore.data.first()
            val raw = prefs[versionVectorKey] ?: return
            val json = JSONObject(raw)
            json.keys().forEach { key ->
                versionVector[key] = json.getString(key)
            }

        } catch (e: Exception) {
            Log.w(TAG, "Failed to load persisted version vector: ${e.message}")
        }
    }

    private suspend fun persistVersionVector() {
        try {
            val json = JSONObject()
            versionVector.forEach { (k, v) -> json.put(k, v) }
            dataStore.edit { prefs ->
                prefs[versionVectorKey] = json.toString()
            }
        } catch (e: Exception) {
            Log.w(TAG, "Failed to persist version vector: ${e.message}")
        }
    }

    private suspend fun loadEntityById(entityId: String): JSONObject? {
        val repo = databaseProvider.arkDataRepository
        // Try todo first
        repo.getTodoById(entityId)?.let { todo ->
            return JSONObject().apply {
                put("type", "todo")
                put("id", todo.id)
                put("data", SyncEntityParser.todoToJson(todo))
                put("hlc", versionVector[todo.id] ?: generateHlc())
            }
        }
        // Try project
        repo.getProjectById(entityId)?.let { project ->
            return JSONObject().apply {
                put("type", "project")
                put("id", project.id)
                put("data", SyncEntityParser.projectToJson(project))
                put("hlc", versionVector[project.id] ?: generateHlc())
            }
        }
        return null
    }

    // -------------------------------------------------------------------------
    // Reconnect
    // -------------------------------------------------------------------------

    private fun scheduleReconnect() {
        if (!isRunning) return
        reconnectJob?.cancel()
        Log.i(TAG, "Scheduling reconnect in ${RECONNECT_DELAY_MS}ms")
        reconnectJob = scope.launch {
            delay(RECONNECT_DELAY_MS)
            if (isRunning) {
                Log.i(TAG, "Reconnecting now...")
                _state.value = LanSyncState.CONNECTING
                doConnect()
            }
        }
    }

    // -------------------------------------------------------------------------
    // HLC utilities
    // -------------------------------------------------------------------------

    private fun compareHlc(a: String, b: String): Int {
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

    private fun generateHlc(): String {
        val now = Instant.now().toString()
        return "$now:000000:$deviceId"
    }

    private fun generateId(): String {
        return "${System.currentTimeMillis()}-${(Math.random() * 1000000).toLong()}"
    }

    private fun jsonArrayToStringList(arr: JSONArray?): List<String> {
        if (arr == null) return emptyList()
        return (0 until arr.length()).map { arr.getString(it) }
    }
}
