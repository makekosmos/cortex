package com.kazui.delphi.data.sync

import android.content.Context
import android.util.Log
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import com.kazui.delphi.data.model.PendingChange
import com.kazui.delphi.di.DatabaseProvider
import io.ktor.client.HttpClient
import io.ktor.client.request.get
import io.ktor.client.request.headers
import io.ktor.client.request.url
import io.ktor.client.statement.bodyAsText
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.decodeFromJsonElement
import kotlinx.serialization.json.encodeToJsonElement
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.longOrNull
import kotlinx.serialization.json.put
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocketListener
import java.time.Instant
import java.util.UUID
import java.util.concurrent.TimeUnit
import javax.inject.Inject
import javax.inject.Singleton
import com.kazui.delphi.data.model.Project
import com.kazui.delphi.data.model.TodoItem

enum class SyncStatus { OFFLINE, SYNCING, ONLINE }

@Singleton
class ArkSyncClient @Inject constructor(
    private val httpClient: HttpClient,
    private val databaseProvider: DatabaseProvider,
    private val dataStore: DataStore<Preferences>,
    private val arkDiscovery: ArkDiscovery,
) {
    private val tag = "ArkSyncClient"
    private val json = Json { ignoreUnknownKeys = true; coerceInputValues = true; encodeDefaults = true }

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    private val _status = MutableStateFlow(SyncStatus.OFFLINE)
    val status: StateFlow<SyncStatus> = _status.asStateFlow()

    private val changeListeners = mutableListOf<(ArkChange) -> Unit>()

    private var currentWs: okhttp3.WebSocket? = null
    private var serverUrl: String = ""
    private var apiKey: String = ""
    private var reconnectJob: Job? = null
    private var deviceSeq = 0L

    // Remote (relay) credentials — preserved for fallback after local discovery
    private var remoteUrl: String = ""
    private var remoteKey: String = ""

    private val okHttpClient = OkHttpClient.Builder()
        .readTimeout(0, TimeUnit.MILLISECONDS)
        .pingInterval(30, TimeUnit.SECONDS)
        .build()

    private val DEVICE_ID_KEY = stringPreferencesKey("ark_device_id")
    private val VERSION_VECTOR_KEY = stringPreferencesKey("ark_version_vector")
    private val ARK_URL_KEY = stringPreferencesKey("ark_url")
    private val ARK_KEY_KEY = stringPreferencesKey("ark_api_key")
    private val SERVER_EPOCH_KEY = stringPreferencesKey("ark_server_epoch")

    init {
        // Auto-connect on startup using saved credentials
        scope.launch {
            val prefs = dataStore.data.first()
            val url = prefs[ARK_URL_KEY]
            val key = prefs[ARK_KEY_KEY]
            if (!url.isNullOrBlank() && !key.isNullOrBlank()) {
                connect(url, key)
            }
        }
    }

    private suspend fun updateVersionVectorBatch(entries: Map<String, Long>) {
        if (entries.isEmpty()) return
        dataStore.edit { prefs ->
            val current = try {
                val stored = prefs[VERSION_VECTOR_KEY] ?: "{}"
                json.parseToJsonElement(stored).jsonObject
            } catch (_: Exception) {
                buildJsonObject {}
            }
            val updated = buildJsonObject {
                current.forEach { (k, v) -> put(k, v) }
                entries.forEach { (k, v) -> put(k, v) }
            }
            prefs[VERSION_VECTOR_KEY] = updated.toString()
        }
    }

    private suspend fun getDeviceId(): String {
        val prefs = dataStore.data.first()
        return prefs[DEVICE_ID_KEY] ?: run {
            val newId = "delphi-android-${UUID.randomUUID()}"
            dataStore.edit { it[DEVICE_ID_KEY] = newId }
            newId
        }
    }

    private suspend fun getVersionVector(): kotlinx.serialization.json.JsonObject {
        val prefs = dataStore.data.first()
        val stored = prefs[VERSION_VECTOR_KEY] ?: return buildJsonObject {}
        return try {
            json.parseToJsonElement(stored).jsonObject
        } catch (_: Exception) {
            buildJsonObject {}
        }
    }

    private suspend fun updateVersionVector(deviceId: String, seq: Long) {
        dataStore.edit { prefs ->
            val current = try {
                val stored = prefs[VERSION_VECTOR_KEY] ?: "{}"
                json.parseToJsonElement(stored).jsonObject
            } catch (_: Exception) {
                buildJsonObject {}
            }
            val updated = buildJsonObject {
                current.forEach { (k, v) -> put(k, v) }
                put(deviceId, seq)
            }
            prefs[VERSION_VECTOR_KEY] = updated.toString()
        }
    }

    fun connect(url: String, key: String) {
        serverUrl = url
        apiKey = key
        // Save as remote credentials for fallback after mDNS loss
        remoteUrl = url
        remoteKey = key
        reconnectJob?.cancel()
        reconnectJob = scope.launch { connectWithRetry() }
    }

    /**
     * Start mDNS discovery for local Ark servers.
     * Call this from an Activity/Service that has a Context.
     * When a local server is found it takes priority; on loss we fall back to remoteUrl.
     */
    fun startDiscovery(context: Context) {
        arkDiscovery.start(
            context = context,
            onFound = { url -> reconnectToLocal(url) },
            onLost = { reconnectToRemote() },
        )
    }

    fun stopDiscovery() {
        arkDiscovery.stop()
    }

    private fun reconnectToLocal(localUrl: String) {
        if (localUrl == serverUrl) {
            Log.d(tag, "Already connected to local $localUrl, skipping reconnect")
            return
        }
        Log.i(tag, "Local Ark found at $localUrl — switching from $serverUrl")
        val currentKey = apiKey.ifBlank { remoteKey }
        serverUrl = localUrl
        apiKey = currentKey
        reconnectJob?.cancel()
        currentWs?.close(1000, "switching to local")
        reconnectJob = scope.launch { connectWithRetry() }
    }

    private fun reconnectToRemote() {
        if (remoteUrl.isBlank()) {
            Log.d(tag, "No remote URL saved, staying offline after local server loss")
            return
        }
        Log.i(tag, "Local Ark lost — falling back to remote $remoteUrl")
        serverUrl = remoteUrl
        apiKey = remoteKey
        reconnectJob?.cancel()
        currentWs?.close(1000, "switching to remote")
        reconnectJob = scope.launch { connectWithRetry() }
    }

    fun disconnect() {
        reconnectJob?.cancel()
        currentWs?.close(1000, "disconnect")
        currentWs = null
        _status.value = SyncStatus.OFFLINE
        serverUrl = ""
        apiKey = ""
    }

    private suspend fun connectWithRetry() {
        var delay = 1000L
        while (true) {
            try {
                _status.value = SyncStatus.SYNCING
                val deviceId = getDeviceId()
                val wsUrl = serverUrl.replace("http://", "ws://").replace("https://", "wss://")
                connectOkHttp("$wsUrl/ws/sync?key=$apiKey", deviceId) {
                    // Called when WS is successfully opened — reset backoff
                    delay = 1000L
                }
            } catch (_: CancellationException) {
                break
            } catch (e: Exception) {
                Log.w(tag, "Connection failed: ${e.message}")
                _status.value = SyncStatus.OFFLINE
                currentWs = null
            }

            if (serverUrl.isEmpty()) break
            Log.d(tag, "Reconnecting in ${delay}ms")
            kotlinx.coroutines.delay(delay)
            delay = minOf(delay * 2, 30_000L)
        }
    }

    private suspend fun connectOkHttp(url: String, deviceId: String, onOpen: () -> Unit) {
        val incoming = Channel<String>(Channel.UNLIMITED)
        val openLatch = CompletableDeferred<okhttp3.WebSocket>()

        val listener = object : WebSocketListener() {
            override fun onOpen(webSocket: okhttp3.WebSocket, response: Response) {
                Log.d(tag, "OkHttp WS opened to $url")
                openLatch.complete(webSocket)
            }

            override fun onMessage(webSocket: okhttp3.WebSocket, text: String) {
                incoming.trySend(text)
            }

            override fun onClosing(webSocket: okhttp3.WebSocket, code: Int, reason: String) {
                Log.d(tag, "OkHttp WS closing: $code $reason")
                webSocket.close(1000, null)
                incoming.close()
            }

            override fun onClosed(webSocket: okhttp3.WebSocket, code: Int, reason: String) {
                Log.d(tag, "OkHttp WS closed: $code $reason")
                incoming.close()
            }

            override fun onFailure(webSocket: okhttp3.WebSocket, t: Throwable, response: Response?) {
                Log.w(tag, "OkHttp WS failure: ${t.message}")
                if (!openLatch.isCompleted) openLatch.completeExceptionally(t)
                incoming.close(t)
            }
        }

        val request = Request.Builder().url(url).build()
        val ws = okHttpClient.newWebSocket(request, listener)

        try {
            // Wait for connection to open (throws on failure)
            val openedWs = openLatch.await()
            currentWs = openedWs
            onOpen()

            // Send sync_start
            val vv = getVersionVector()
            openedWs.send(json.encodeToString(buildJsonObject {
                put("type", "sync_start")
                put("device_id", deviceId)
                put("device_name", "Delphi Android")
                put("platform", "android")
                put("vector", vv)
            }))
            Log.d(tag, "sync_start sent")

            // Message loop — runs until channel is closed (WS closed/failed)
            for (text in incoming) {
                handleMessage(text, deviceId)
            }
            Log.d(tag, "OkHttp WS message loop ended")
        } catch (e: CancellationException) {
            ws.close(1000, "cancelled")
            throw e
        } catch (e: Exception) {
            ws.cancel()
            throw e
        } finally {
            currentWs = null
            _status.value = SyncStatus.OFFLINE
        }
    }

    private suspend fun handleMessage(text: String, deviceId: String) {
        try {
            val msg = json.parseToJsonElement(text).jsonObject
            when (msg["type"]?.jsonPrimitive?.content) {
                "sync_changes" -> {
                    _status.value = SyncStatus.ONLINE
                    val changes = msg["changes"]?.jsonArray ?: return
                    val isFullSync = msg["is_full_sync"]?.jsonPrimitive?.booleanOrNull ?: false
                    val incomingEpoch = msg["server_epoch"]?.jsonPrimitive?.contentOrNull
                    val storedEpoch = dataStore.data.first()[SERVER_EPOCH_KEY]
                    val epochChanged = incomingEpoch != null && storedEpoch != null && incomingEpoch != storedEpoch

                    if (epochChanged) {
                        // Server DB was wiped — reset our vector so we push full local state.
                        // Do NOT delete any local data: the client is the authority now.
                        Log.w(tag, "Server epoch changed ($storedEpoch -> $incomingEpoch) — pushing full local state")
                        dataStore.edit { prefs -> prefs.remove(VERSION_VECTOR_KEY) }
                        deviceSeq = 0L
                    }
                    if (incomingEpoch != null) {
                        dataStore.edit { prefs -> prefs[SERVER_EPOCH_KEY] = incomingEpoch }
                    }

                    val todosToUpsert = mutableListOf<TodoItem>()
                    val taskIdsToDelete = mutableListOf<String>()
                    val projectsToUpsert = mutableListOf<Project>()
                    val projectIdsToDelete = mutableListOf<String>()
                    val maxSeqPerDevice = mutableMapOf<String, Long>()
                    val serverTaskIds = mutableSetOf<String>()

                    changes.forEach { elem ->
                        try {
                            val change = json.decodeFromJsonElement<ArkChange>(elem)
                            val did = change.device_id
                            val seq = change.device_seq
                            if (did != null && seq != null) {
                                maxSeqPerDevice[did] = maxOf(maxSeqPerDevice[did] ?: 0L, seq)
                            }
                            when {
                                ArkEventMapper.isTaskChange(change) -> {
                                    serverTaskIds.add(change.data.source_id.lowercase())
                                    if (change.change_type == "delete") {
                                        taskIdsToDelete.add(change.data.source_id)
                                    } else {
                                        ArkEventMapper.arkChangeToTodoItem(change)?.let { todosToUpsert.add(it) }
                                    }
                                }
                                ArkEventMapper.isProjectChange(change) -> {
                                    if (change.change_type == "delete") {
                                        projectIdsToDelete.add(change.data.source_id)
                                    } else {
                                        ArkEventMapper.arkChangeToProject(change)?.let { projectsToUpsert.add(it) }
                                    }
                                }
                            }
                            if (did != deviceId) {
                                changeListeners.forEach { it(change) }
                            }
                        } catch (e: Exception) {
                            Log.w(tag, "Skipping malformed change: ${e.message}")
                        }
                    }

                    val repo = databaseProvider.arkDataRepository
                    if (projectsToUpsert.isNotEmpty()) {
                        try { repo.upsertProjects(projectsToUpsert) }
                        catch (e: Exception) { Log.e(tag, "upsertProjects failed: ${e.message}") }
                    }
                    projectIdsToDelete.forEach { id ->
                        try { repo.deleteProjectById(id) }
                        catch (e: Exception) { Log.e(tag, "deleteProject failed: ${e.message}") }
                    }
                    if (todosToUpsert.isNotEmpty()) {
                        try { repo.upsertAll(todosToUpsert) }
                        catch (e: Exception) {
                            Log.w(tag, "Batch upsert failed, falling back to per-item: ${e.message}")
                            todosToUpsert.forEach { todo ->
                                try { repo.upsert(todo) }
                                catch (ex: Exception) { Log.w(tag, "Skipping todo ${todo.id}: ${ex.message}") }
                            }
                        }
                    }
                    taskIdsToDelete.forEach { id ->
                        try { repo.deleteById(id) }
                        catch (e: Exception) { Log.e(tag, "deleteTodo failed: ${e.message}") }
                    }

                    updateVersionVectorBatch(maxSeqPerDevice)
                    flushOutbox(deviceId)

                    if ((isFullSync || epochChanged) && serverTaskIds.isNotEmpty()) {
                        // Zombie cleanup ONLY when epoch matches — on server wipe we push
                        // local data rather than deleting it (client is the authority then).
                        if (isFullSync && !epochChanged) {
                            val syncRepo = databaseProvider.arkDataRepository
                            val outboxIds = if (databaseProvider.isOpen) databaseProvider.pendingChangeDao().getAll() else emptyList<PendingChange>()
                                .mapNotNull { pending ->
                                    try { json.decodeFromString<ArkChange>(pending.payload).data.source_id.lowercase().ifBlank { null } }
                                    catch (_: Exception) { null }
                                }.toSet()
                            val allLocal = syncRepo.getAllForSync()
                            val zombies = allLocal.filter {
                                it.id.lowercase() !in serverTaskIds && it.id.lowercase() !in outboxIds
                            }
                            if (zombies.isNotEmpty()) {
                                Log.i(tag, "Removing ${zombies.size} zombie tasks after full sync")
                                zombies.forEach { zombie ->
                                    try { syncRepo.deleteById(zombie.id) }
                                    catch (e: Exception) { Log.w(tag, "Failed to delete zombie ${zombie.id}: ${e.message}") }
                                }
                            }
                        }
                        // Push local tasks server doesn't have (works for both normal full sync and epoch change)
                        sendMissingToServer(serverTaskIds, deviceId)
                    }
                }
                "change" -> {
                    val change = json.decodeFromJsonElement<ArkChange>(msg)
                    applyChangeToDb(change)
                    val did = change.device_id
                    val seq = change.device_seq
                    if (did != null && seq != null) updateVersionVector(did, seq)
                    if (change.device_id != deviceId) {
                        changeListeners.forEach { it(change) }
                    }
                }
                "change_ack" -> {
                    val seq = msg["device_seq"]?.jsonPrimitive?.longOrNull ?: return
                    updateVersionVector(deviceId, seq)
                }
                "ping" -> {
                    currentWs?.send(json.encodeToString(buildJsonObject {
                        put("type", "pong")
                    }))
                }
                "error" -> {
                    Log.e(tag, "Server error: ${msg["message"]?.jsonPrimitive?.content}")
                }
            }
        } catch (e: Exception) {
            Log.e(tag, "Error handling message: ${e.message}")
        }
    }

    private suspend fun applyChangeToDb(change: ArkChange) {
        try {
            val repo = databaseProvider.arkDataRepository
            when {
                ArkEventMapper.isTaskChange(change) -> {
                    if (change.change_type == "delete") {
                        repo.deleteById(change.data.source_id)
                    } else {
                        ArkEventMapper.arkChangeToTodoItem(change)?.let { repo.upsert(it) }
                    }
                }
                ArkEventMapper.isProjectChange(change) -> {
                    if (change.change_type == "delete") {
                        repo.deleteProjectById(change.data.source_id)
                    } else {
                        ArkEventMapper.arkChangeToProject(change)?.let { repo.upsertProject(it) }
                    }
                }
            }
        } catch (e: Exception) {
            Log.e(tag, "applyChangeToDb failed: ${e.message}")
        }
    }

    fun sendChange(change: ArkChange) {
        scope.launch {
            val ws = currentWs
            if (ws != null && _status.value == SyncStatus.ONLINE) {
                try {
                    val deviceId = getDeviceId()
                    deviceSeq++
                    val withDeviceInfo = change.copy(
                        device_id = deviceId,
                        device_seq = deviceSeq,
                    )
                    val payload = json.encodeToJsonElement(withDeviceInfo).jsonObject
                        .let { obj ->
                            buildJsonObject {
                                put("type", "change")
                                obj.forEach { (k, v) -> put(k, v) }
                            }
                        }
                    ws.send(json.encodeToString(payload))
                } catch (_: Exception) {
                    queueChange(change)
                }
            } else {
                queueChange(change)
            }
        }
    }

    private suspend fun queueChange(change: ArkChange) {
        if (!databaseProvider.isOpen) return
        try {
            databaseProvider.pendingChangeDao().insert(
                PendingChange(
                    payload = json.encodeToString(change),
                    createdAt = Instant.now().toString(),
                )
            )
        } catch (e: Exception) {
            Log.w(tag, "queueChange failed (DB closed?): ${e.message}")
        }
    }

    private suspend fun flushOutbox(deviceId: String) {
        if (!databaseProvider.isOpen) return
        val pending = databaseProvider.pendingChangeDao().getAll()
        pending.forEach { pendingChange ->
            try {
                val change = json.decodeFromString<ArkChange>(pendingChange.payload)
                // Re-read current state from Room to avoid sending stale pre-sync data.
                // If a task was updated by sync_changes (e.g. completed on another device),
                // we must send the current state, not the old cached payload.
                val currentChange = if (change.change_type != "delete" && change.data.event_type == "task") {
                    val taskId = change.data.source_id.ifBlank { change.event_id }
                    databaseProvider.arkDataRepository.getTodoById(taskId)?.let { ArkEventMapper.todoToArkChange(it, change.change_type, deviceId) }
                        ?: change
                } else {
                    change
                }
                deviceSeq++
                val withDeviceInfo = currentChange.copy(device_id = deviceId, device_seq = deviceSeq)
                val payload = json.encodeToJsonElement(withDeviceInfo).jsonObject
                    .let { obj -> buildJsonObject { put("type", "change"); obj.forEach { (k, v) -> put(k, v) } } }
                currentWs?.send(json.encodeToString(payload))
                databaseProvider.pendingChangeDao().deleteById(pendingChange.id)
            } catch (e: Exception) {
                Log.e(tag, "Failed to flush outbox item: ${e.message}")
            }
        }
    }

    private suspend fun sendMissingToServer(serverKnownIds: Set<String>, deviceId: String) {
        val ws = currentWs ?: return
        val allTasks = databaseProvider.arkDataRepository.getAllForSync()
        val missing = allTasks.filter { it.id.lowercase() !in serverKnownIds }
        if (missing.isEmpty()) {
            Log.d(tag, "No missing tasks to send to server")
            return
        }
        Log.i(tag, "Sending ${missing.size} missing tasks to server")
        val batchChanges = missing.map { task ->
            deviceSeq++
            ArkEventMapper.todoToArkChange(task, "create", deviceId).copy(
                device_id = deviceId,
                device_seq = deviceSeq,
            )
        }
        val batchMsg = buildJsonObject {
            put("type", "sync_changes")
            put("changes", json.encodeToJsonElement(batchChanges))
        }
        ws.send(json.encodeToString(batchMsg))
    }

    fun clearLocalData() {
        scope.launch {
            // Disconnect
            currentWs?.close(1000, "clear local data")
            currentWs = null
            reconnectJob?.cancel()
            _status.value = SyncStatus.OFFLINE

            // Clear all local DB tables (via ContentProvider)
            val clearRepo = databaseProvider.arkDataRepository
            clearRepo.deleteAll()
            clearRepo.deleteAllChecklistItems()
            clearRepo.deleteAllTagRefs()
            clearRepo.deleteAllProjects()
            clearRepo.deleteAllAreas()
            clearRepo.deleteAllTags()
            clearRepo.deleteAllHeadings()
            if (databaseProvider.isOpen) {
                try { databaseProvider.pendingChangeDao().deleteAll() } catch (_: Exception) {}
            }

            // Reset sync state
            dataStore.edit { prefs ->
                prefs.remove(VERSION_VECTOR_KEY)
                prefs.remove(SERVER_EPOCH_KEY)
            }
            deviceSeq = 0L

            Log.i(tag, "Local data cleared, reconnecting...")

            // Reconnect — empty vector means full sync
            val url = serverUrl.ifBlank { remoteUrl }
            val key = apiKey.ifBlank { remoteKey }
            if (url.isNotBlank() && key.isNotBlank()) {
                connect(url, key)
            }
        }
    }

    fun resetAndResync() {
        scope.launch {
            dataStore.edit { prefs -> prefs.remove(VERSION_VECTOR_KEY) }
            deviceSeq = 0L
            currentWs?.close(1000, "resync")
            currentWs = null
        }
    }

    fun onChange(handler: (ArkChange) -> Unit): () -> Unit {
        changeListeners.add(handler)
        return { changeListeners.remove(handler) }
    }

    suspend fun fetchTasksFromArk(
        serverUrl: String,
        apiKey: String,
    ): List<com.kazui.delphi.data.model.TodoItem> {
        return try {
            val response = httpClient.get {
                url("$serverUrl/events") {
                    parameters.append("event_type", "task")
                    parameters.append("limit", "1000")
                }
                headers { append("X-API-Key", apiKey) }
            }
            val body = response.bodyAsText()
            val parsed = json.parseToJsonElement(body).jsonArray
            parsed.mapNotNull { elem ->
                try {
                    val event = elem.jsonObject
                    val eventType = event["event_type"]?.jsonPrimitive?.contentOrNull ?: return@mapNotNull null
                    if (eventType != "task" && eventType != "task_created") return@mapNotNull null
                    val innerData = event["data"]?.jsonObject ?: return@mapNotNull null
                    val sourceId = event["source_id"]?.jsonPrimitive?.contentOrNull ?: ""
                    val summary = event["summary"]?.jsonPrimitive?.contentOrNull ?: ""
                    val occurredAt = event["occurred_at"]?.jsonPrimitive?.contentOrNull ?: ""
                    val fakeChange = ArkChange(
                        event_id = event["id"]?.jsonPrimitive?.contentOrNull ?: sourceId,
                        change_type = "create",
                        data = ArkEventData(
                            event_type = eventType,
                            source_id = sourceId,
                            summary = summary,
                            occurred_at = occurredAt,
                            data = innerData,
                        ),
                    )
                    ArkEventMapper.arkChangeToTodoItem(fakeChange)
                } catch (_: Exception) { null }
            }
        } catch (e: Exception) {
            Log.e(tag, "fetchTasksFromArk failed: ${e.message}")
            emptyList()
        }
    }
}
