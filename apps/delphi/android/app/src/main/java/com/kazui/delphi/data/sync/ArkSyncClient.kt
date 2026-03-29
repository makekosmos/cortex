package com.kazui.delphi.data.sync

import android.util.Log
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import com.kazui.delphi.data.db.PendingChangeDao
import com.kazui.delphi.data.model.PendingChange
import io.ktor.client.HttpClient
import io.ktor.client.plugins.websocket.webSocket
import io.ktor.client.request.get
import io.ktor.client.request.headers
import io.ktor.client.request.url
import io.ktor.client.statement.bodyAsText
import io.ktor.websocket.DefaultWebSocketSession
import io.ktor.websocket.Frame
import io.ktor.websocket.close
import io.ktor.websocket.readText
import io.ktor.websocket.send
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.encodeToJsonElement
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.decodeFromJsonElement
import kotlinx.serialization.json.contentOrNull
import java.util.UUID
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.longOrNull
import kotlinx.serialization.json.put
import java.time.Instant
import javax.inject.Inject
import javax.inject.Singleton
import com.kazui.delphi.data.model.Project
import com.kazui.delphi.data.model.TodoItem

enum class SyncStatus { OFFLINE, SYNCING, ONLINE }

@Singleton
class ArkSyncClient @Inject constructor(
    private val httpClient: HttpClient,
    private val pendingChangeDao: PendingChangeDao,
    private val todoDao: com.kazui.delphi.data.db.TodoDao,
    private val projectDao: com.kazui.delphi.data.db.ProjectDao,
    private val dataStore: DataStore<Preferences>,
) {
    private val tag = "ArkSyncClient"
    private val json = Json { ignoreUnknownKeys = true; coerceInputValues = true }

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    private val _status = MutableStateFlow(SyncStatus.OFFLINE)
    val status: StateFlow<SyncStatus> = _status.asStateFlow()

    private val changeListeners = mutableListOf<(ArkChange) -> Unit>()

    private var currentSession: DefaultWebSocketSession? = null
    private var serverUrl: String = ""
    private var apiKey: String = ""
    private var reconnectJob: Job? = null
    private var deviceSeq = 0L

    private val DEVICE_ID_KEY = stringPreferencesKey("ark_device_id")
    private val VERSION_VECTOR_KEY = stringPreferencesKey("ark_version_vector")

    // Update multiple device entries in the vector in a single DataStore write
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
        reconnectJob?.cancel()
        reconnectJob = scope.launch { connectWithRetry() }
    }

    fun disconnect() {
        reconnectJob?.cancel()
        scope.launch {
            currentSession?.close()
            currentSession = null
        }
        _status.value = SyncStatus.OFFLINE
        serverUrl = ""
        apiKey = ""
    }

    private suspend fun connectWithRetry() {
        var delay = 1000L
        while (true) {
            try {
                _status.value = SyncStatus.SYNCING
                val wsUrl = serverUrl
                    .replace("http://", "ws://")
                    .replace("https://", "wss://")
                httpClient.webSocket("$wsUrl/ws/sync?key=$apiKey") {
                    currentSession = this
                    val deviceId = getDeviceId()
                    val versionVector = getVersionVector()

                    // Send sync_start
                    send(json.encodeToString(buildJsonObject {
                        put("type", "sync_start")
                        put("device_id", deviceId)
                        put("device_name", "Delphi Android")
                        put("platform", "android")
                        put("vector", versionVector)
                    }))

                    delay = 1000L // reset backoff on successful connection

                    for (frame in incoming) {
                        if (frame is Frame.Text) {
                            handleMessage(frame.readText(), deviceId)
                        }
                    }
                }
            } catch (_: CancellationException) {
                break
            } catch (e: Exception) {
                Log.w(tag, "Connection failed: ${e.message}")
                _status.value = SyncStatus.OFFLINE
                currentSession = null
            }

            if (serverUrl.isEmpty()) break
            Log.d(tag, "Reconnecting in ${delay}ms")
            kotlinx.coroutines.delay(delay)
            delay = minOf(delay * 2, 30_000L)
        }
    }

    private suspend fun handleMessage(text: String, deviceId: String) {
        try {
            val msg = json.parseToJsonElement(text).jsonObject
            when (msg["type"]?.jsonPrimitive?.content) {
                "sync_changes" -> {
                    _status.value = SyncStatus.ONLINE
                    val changes = msg["changes"]?.jsonArray ?: return

                    // Collect into batches for efficient DB writes
                    val todosToUpsert = mutableListOf<TodoItem>()
                    val taskIdsToDelete = mutableListOf<String>()
                    val projectsToUpsert = mutableListOf<Project>()
                    val projectIdsToDelete = mutableListOf<String>()
                    val maxSeqPerDevice = mutableMapOf<String, Long>()

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

                    // Batch DB writes — projects FIRST (todos have FK to projects)
                    if (projectsToUpsert.isNotEmpty()) {
                        try { projectDao.upsertProjects(projectsToUpsert) }
                        catch (e: Exception) { Log.e(tag, "upsertProjects failed: ${e.message}") }
                    }
                    projectIdsToDelete.forEach { id ->
                        try { projectDao.deleteProjectById(id) }
                        catch (e: Exception) { Log.e(tag, "deleteProject failed: ${e.message}") }
                    }
                    if (todosToUpsert.isNotEmpty()) {
                        try { todoDao.upsertAll(todosToUpsert) }
                        catch (e: Exception) {
                            // FK violations on batch — fall back to per-item upserts
                            Log.w(tag, "Batch upsert failed, falling back to per-item: ${e.message}")
                            todosToUpsert.forEach { todo ->
                                try { todoDao.upsert(todo) }
                                catch (ex: Exception) { Log.w(tag, "Skipping todo ${todo.id}: ${ex.message}") }
                            }
                        }
                    }
                    taskIdsToDelete.forEach { id ->
                        try { todoDao.deleteById(id) }
                        catch (e: Exception) { Log.e(tag, "deleteTodo failed: ${e.message}") }
                    }

                    // Single vector write for all devices seen
                    updateVersionVectorBatch(maxSeqPerDevice)

                    flushOutbox(deviceId)
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
                    currentSession?.send(json.encodeToString(buildJsonObject {
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
            when {
                ArkEventMapper.isTaskChange(change) -> {
                    if (change.change_type == "delete") {
                        todoDao.deleteById(change.data.source_id)
                    } else {
                        ArkEventMapper.arkChangeToTodoItem(change)?.let { todoDao.upsert(it) }
                    }
                }
                ArkEventMapper.isProjectChange(change) -> {
                    if (change.change_type == "delete") {
                        projectDao.deleteProjectById(change.data.source_id)
                    } else {
                        ArkEventMapper.arkChangeToProject(change)?.let { projectDao.upsertProject(it) }
                    }
                }
            }
        } catch (e: Exception) {
            Log.e(tag, "applyChangeToDb failed: ${e.message}")
        }
    }

    fun sendChange(change: ArkChange) {
        scope.launch {
            val session = currentSession
            if (session != null && _status.value == SyncStatus.ONLINE) {
                try {
                    val deviceId = getDeviceId()
                    deviceSeq++
                    val withDeviceInfo = change.copy(
                        device_id = deviceId,
                        device_seq = deviceSeq,
                    )
                    // Ark WS expects top-level "type": "change"
                    val payload = json.encodeToJsonElement(withDeviceInfo).jsonObject
                        .let { obj ->
                            buildJsonObject {
                                put("type", "change")
                                obj.forEach { (k, v) -> put(k, v) }
                            }
                        }
                    session.send(json.encodeToString(payload))
                } catch (_: Exception) {
                    queueChange(change)
                }
            } else {
                queueChange(change)
            }
        }
    }

    private suspend fun queueChange(change: ArkChange) {
        pendingChangeDao.insert(
            PendingChange(
                payload = json.encodeToString(change),
                createdAt = Instant.now().toString(),
            )
        )
    }

    private suspend fun flushOutbox(deviceId: String) {
        val pending = pendingChangeDao.getAll()
        pending.forEach { pendingChange ->
            try {
                val change = json.decodeFromString<ArkChange>(pendingChange.payload)
                deviceSeq++
                val withDeviceInfo = change.copy(device_id = deviceId, device_seq = deviceSeq)
                val payload = json.encodeToJsonElement(withDeviceInfo).jsonObject
                    .let { obj -> buildJsonObject { put("type", "change"); obj.forEach { (k, v) -> put(k, v) } } }
                currentSession?.send(json.encodeToString(payload))
                pendingChangeDao.deleteById(pendingChange.id)
            } catch (e: Exception) {
                Log.e(tag, "Failed to flush outbox item: ${e.message}")
            }
        }
    }

    /**
     * Clear the stored version vector so the next connection triggers a full resync
     * from the server. Call this when local DB state is suspected to be stale.
     */
    fun resetAndResync() {
        scope.launch {
            dataStore.edit { prefs -> prefs.remove(VERSION_VECTOR_KEY) }
            deviceSeq = 0L
            // Close current session — connectWithRetry will reopen it
            currentSession?.close()
            currentSession = null
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
                    // HTTP /events format differs from WS ArkChange format:
                    // {id, event_type, source_id, summary, occurred_at, data: {task fields}}
                    // Wrap into ArkChange-compatible structure for the mapper
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
