package com.kazui.delphi.data.sync

import android.content.Context
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import android.net.wifi.WifiManager
import android.util.Log
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.serialization.json.Json
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import org.json.JSONArray
import org.json.JSONObject
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.TimeUnit
import javax.inject.Inject
import javax.inject.Singleton

private const val TAG = "ArkPeerManager"
private const val SERVICE_TYPE = "_ark-peer._tcp"
private const val RECONNECT_DELAY_MS = 15_000L

@Singleton
class ArkPeerManager @Inject constructor(
    private val databaseProvider: com.kazui.delphi.di.DatabaseProvider,
) {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    private var meshSecret: String = ""
    private var meshId: String = ""
    private var deviceId: String = ""
    private var deviceName: String = ""

    @Volatile private var isRunning = false

    private val okHttpClient = OkHttpClient.Builder()
        .pingInterval(30, TimeUnit.SECONDS)
        .build()

    // Active outbound connections: peerDeviceId -> WebSocket
    private val connections = ConcurrentHashMap<String, WebSocket>()
    // Reconnect jobs
    private val reconnectJobs = ConcurrentHashMap<String, Job>()

    // NSD
    private var nsdManager: NsdManager? = null
    private var appContext: Context? = null
    private var discoveryListener: NsdManager.DiscoveryListener? = null
    private var multicastLock: WifiManager.MulticastLock? = null
    @Volatile private var isDiscoveryActive = false
    private var discoveryRestartJob: Job? = null
    private companion object {
        const val DISCOVERY_RESTART_INTERVAL_MS = 30_000L
    }

    // Callback for received changes
    var onChangeReceived: ((ArkChange) -> Unit)? = null
    // Callback when a peer connects (informational)
    var onPeerConnected: ((String) -> Unit)? = null

    fun start(context: Context, meshSecret: String, deviceId: String, deviceName: String) {
        if (isRunning) stop()
        this.meshSecret = meshSecret
        this.meshId = ArkPeerProtocol.computeMeshId(meshSecret)
        this.deviceId = deviceId
        this.deviceName = deviceName
        this.appContext = context.applicationContext
        isRunning = true
        startDiscovery(context)
        scheduleDiscoveryRestart()
        Log.i(TAG, "P2P mesh started (meshId=${meshId.take(8)}...)")
    }

    fun stop() {
        isRunning = false
        discoveryRestartJob?.cancel()
        discoveryRestartJob = null
        stopDiscovery()
        releaseMulticastLock()
        reconnectJobs.values.forEach { it.cancel() }
        reconnectJobs.clear()
        connections.values.forEach { it.close(1000, "stopping") }
        connections.clear()
        appContext = null
        Log.i(TAG, "P2P mesh stopped")
    }

    fun broadcastChange(change: ArkChange) {
        if (connections.isEmpty()) return
        val json = buildChangeJson(change)
        connections.forEach { (_, ws) ->
            ws.send(json)
        }
    }

    // MARK: - mDNS Discovery

    private fun acquireMulticastLock(context: Context) {
        if (multicastLock?.isHeld == true) return
        val wifiManager = context.applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager
        multicastLock = wifiManager.createMulticastLock("ArkPeerMDNS").apply {
            setReferenceCounted(false)
            acquire()
        }

    }

    private fun releaseMulticastLock() {
        multicastLock?.let {
            if (it.isHeld) {
                it.release()

            }
        }
        multicastLock = null
    }

    private fun startDiscovery(context: Context) {
        if (isDiscoveryActive) return
        acquireMulticastLock(context)
        nsdManager = context.getSystemService(Context.NSD_SERVICE) as NsdManager

        discoveryListener = object : NsdManager.DiscoveryListener {
            override fun onStartDiscoveryFailed(serviceType: String, errorCode: Int) {
                Log.e(TAG, "Discovery failed to start: $errorCode")
                isDiscoveryActive = false
            }
            override fun onStopDiscoveryFailed(serviceType: String, errorCode: Int) {
                Log.e(TAG, "Discovery failed to stop: $errorCode")
            }
            override fun onDiscoveryStarted(serviceType: String) {
                isDiscoveryActive = true
            }
            override fun onDiscoveryStopped(serviceType: String) {
                isDiscoveryActive = false
            }
            override fun onServiceFound(serviceInfo: NsdServiceInfo) {
                resolveService(serviceInfo)
            }
            override fun onServiceLost(serviceInfo: NsdServiceInfo) { }
        }

        try {
            nsdManager?.discoverServices(SERVICE_TYPE, NsdManager.PROTOCOL_DNS_SD, discoveryListener)
        } catch (e: Exception) {
            Log.e(TAG, "Failed to start discovery: ${e.message}")
        }
    }

    private fun stopDiscovery() {
        val listener = discoveryListener
        if (listener != null && isDiscoveryActive) {
            try {
                nsdManager?.stopServiceDiscovery(listener)
            } catch (e: Exception) {
                Log.e(TAG, "stopServiceDiscovery threw: ${e.message}")
            }
        }
        discoveryListener = null
        nsdManager = null
        isDiscoveryActive = false
    }

    private fun resolveService(serviceInfo: NsdServiceInfo, retryCount: Int = 0) {
        val resolveListener = object : NsdManager.ResolveListener {
            override fun onResolveFailed(info: NsdServiceInfo, errorCode: Int) {
                Log.w(TAG, "Resolve failed: ${info.serviceName} code=$errorCode (attempt ${retryCount + 1})")
                // Retry up to 3 times with a delay (NSD resolve can fail transiently)
                if (retryCount < 3 && isRunning) {
                    scope.launch {
                        delay(1000L * (retryCount + 1))
                        resolveService(serviceInfo, retryCount + 1)
                    }
                }
            }
            override fun onServiceResolved(info: NsdServiceInfo) {
                val host = info.host?.hostAddress ?: return
                val port = info.port.takeIf { it > 0 } ?: return
                // Extract TXT record attributes for mesh_id and device_id filtering
                val attrs = info.attributes
                val peerMeshId = attrs["mesh_id"]?.let { String(it, Charsets.UTF_8) } ?: ""
                val peerDeviceId = attrs["device_id"]?.let { String(it, Charsets.UTF_8) } ?: info.serviceName

                // Filter by mesh_id — only connect to peers in same mesh
                if (peerMeshId.isNotEmpty() && peerMeshId != meshId) {
                    return
                }
                // Skip self
                if (peerDeviceId == deviceId) return
                // Skip if already connected
                if (connections.containsKey(peerDeviceId)) return

                connectToPeer(host, port, peerDeviceId)
            }
        }

        try {
            nsdManager?.resolveService(serviceInfo, resolveListener)
        } catch (e: Exception) {
            Log.e(TAG, "resolveService threw: ${e.message}")
        }
    }

    /** Periodically restart NSD discovery to work around Android NSD flakiness. */
    private fun scheduleDiscoveryRestart() {
        discoveryRestartJob?.cancel()
        discoveryRestartJob = scope.launch {
            while (isRunning) {
                delay(DISCOVERY_RESTART_INTERVAL_MS)
                if (!isRunning) break
                val ctx = appContext ?: break
                // Only restart if we have no connections (peers not found yet)
                if (connections.isEmpty()) {
                    stopDiscovery()
                    delay(500) // brief pause before restarting
                    startDiscovery(ctx)
                }
            }
        }
    }

    /**
     * Manually connect to a peer by IP and port (bypasses mDNS).
     * Useful when NSD discovery is unreliable.
     */
    fun connectDirectly(host: String, port: Int) {
        if (!isRunning) return
        val peerId = "direct-$host:$port"
        if (connections.containsKey(peerId)) return
        Log.i(TAG, "Direct connect to $host:$port")
        connectToPeer(host, port, peerId)
    }

    // MARK: - Outbound WebSocket

    private fun connectToPeer(host: String, port: Int, peerDeviceId: String) {
        if (!isRunning) return
        if (connections.containsKey(peerDeviceId)) return

        val request = Request.Builder()
            .url("ws://$host:$port")
            .build()

        okHttpClient.newWebSocket(request, object : WebSocketListener() {
            private var authenticated = false

            override fun onOpen(webSocket: WebSocket, response: Response) {
                val nonce = ArkPeerProtocol.generateNonce()
                val hello = JSONObject().apply {
                    put("type", "peer_hello")
                    put("protocol_version", 2)
                    put("device_id", deviceId)
                    put("device_name", deviceName)
                    put("platform", "android")
                    put("mesh_id", meshId)
                    put("nonce", nonce)
                    put("auth_hmac", ArkPeerProtocol.computeAuthHmac(meshSecret, nonce))
                }
                webSocket.send(hello.toString())
            }

            override fun onMessage(webSocket: WebSocket, text: String) {
                try {
                    val msg = JSONObject(text)
                    val type = msg.optString("type")

                    if (!authenticated) {
                        if (type != "peer_hello_ack") {
                            Log.w(TAG, "Expected peer_hello_ack, got $type")
                            webSocket.close(1002, "expected ack")
                            return
                        }
                        val ok = msg.optBoolean("ok", false)
                        if (!ok) {
                            Log.w(TAG, "Peer rejected: ${msg.optString("error")}")
                            webSocket.close(1002, "rejected")
                            return
                        }
                        // Verify ack HMAC if present
                        val ackNonce = msg.optString("nonce")
                        val ackHmac = msg.optString("auth_hmac")
                        if (ackNonce.isNotEmpty() && ackHmac.isNotEmpty()) {
                            if (!ArkPeerProtocol.verifyAuthHmac(meshSecret, ackNonce, ackHmac)) {
                                Log.w(TAG, "Ack HMAC verification failed")
                                webSocket.close(1002, "auth failed")
                                return
                            }
                        }
                        authenticated = true
                        connections[peerDeviceId] = webSocket
                        Log.i(TAG, "Authenticated peer: $peerDeviceId")
                        onPeerConnected?.invoke(peerDeviceId)
                        pushLocalState(webSocket)
                        return
                    }

                    if (type == "change") {
                        handleIncomingChange(msg, peerDeviceId, webSocket)
                    }
                } catch (e: Exception) {
                    Log.e(TAG, "Error handling message: ${e.message}")
                }
            }

            override fun onClosed(webSocket: WebSocket, code: Int, reason: String) {
                connections.remove(peerDeviceId)
                Log.i(TAG, "Peer $peerDeviceId disconnected: $reason")
                scheduleReconnect(host, port, peerDeviceId)
            }

            override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
                connections.remove(peerDeviceId)
                Log.w(TAG, "Peer $peerDeviceId connection failed: ${t.message}")
                scheduleReconnect(host, port, peerDeviceId)
            }
        })

    }

    private fun scheduleReconnect(host: String, port: Int, peerDeviceId: String) {
        if (!isRunning) return
        if (reconnectJobs.containsKey(peerDeviceId)) return
        reconnectJobs[peerDeviceId] = scope.launch {
            delay(RECONNECT_DELAY_MS)
            reconnectJobs.remove(peerDeviceId)
            if (isRunning && !connections.containsKey(peerDeviceId)) {
                connectToPeer(host, port, peerDeviceId)
            }
        }
    }

    // MARK: - Change handling

    private fun handleIncomingChange(msg: JSONObject, fromDevice: String, fromWs: WebSocket) {
        try {
            // Extract hop_path and add self for loop prevention
            val hopPathJson = msg.optJSONArray("hop_path")
            val hopPath = mutableListOf<String>()
            if (hopPathJson != null) {
                for (i in 0 until hopPathJson.length()) {
                    hopPath.add(hopPathJson.getString(i))
                }
            }
            if (!hopPath.contains(deviceId)) hopPath.add(deviceId)

            // Parse into ArkChange and notify
            val dataJson = msg.optJSONObject("data")
            if (dataJson != null) {
                val eventType = dataJson.optString("event_type", "task")
                val sourceId = dataJson.optString("source_id", "")
                val summary = dataJson.optString("summary", "")
                val occurredAt = dataJson.optString("occurred_at", "")
                val innerData = dataJson.optJSONObject("data")

                val innerDataJsonObject = buildInnerDataJsonObject(innerData)
                val arkChange = ArkChange(
                    event_id = msg.optString("event_id", sourceId).lowercase(),
                    change_type = msg.optString("change_type", "update"),
                    data = ArkEventData(
                        event_type = eventType,
                        source_id = sourceId.lowercase().ifEmpty {
                            msg.optString("event_id").lowercase()
                        },
                        summary = summary,
                        occurred_at = occurredAt,
                        data = innerDataJsonObject,
                    ),
                )
                onChangeReceived?.invoke(arkChange)
            }

            // Re-broadcast to other peers (hop_path loop prevention)
            msg.put("hop_path", JSONArray(hopPath))
            val forwardText = msg.toString()
            connections.forEach { (peerId, ws) ->
                if (peerId != fromDevice && !hopPath.contains(peerId) && ws != fromWs) {
                    ws.send(forwardText)
                }
            }
        } catch (e: Exception) {
            Log.e(TAG, "Error processing incoming change: ${e.message}")
        }
    }

    private fun buildInnerDataJsonObject(json: JSONObject?): kotlinx.serialization.json.JsonObject {
        if (json == null) return kotlinx.serialization.json.JsonObject(emptyMap())
        return try {
            Json.decodeFromString<kotlinx.serialization.json.JsonObject>(json.toString())
        } catch (e: Exception) {
            kotlinx.serialization.json.JsonObject(emptyMap())
        }
    }

    // MARK: - Initial state push

    private fun pushLocalState(webSocket: WebSocket) {
        scope.launch {
            try {
                val todos = databaseProvider.todoDao().getAllForSync()
                val projects = databaseProvider.projectDao().getAllForSync()
                var seq = 0
                todos.forEach { todo ->
                    val change = ArkEventMapper.todoToArkChange(todo, "update", deviceId)
                    webSocket.send(buildChangeJson(change, seq++))
                }
                projects.forEach { project ->
                    val change = ArkEventMapper.projectToArkChange(project, "update", deviceId)
                    webSocket.send(buildChangeJson(change, seq++))
                }
                Log.i(TAG, "Pushed ${todos.size} todos + ${projects.size} projects to peer")
            } catch (e: Exception) {
                Log.e(TAG, "Failed to push local state: ${e.message}")
            }
        }
    }

    private fun buildChangeJson(change: ArkChange, seq: Int = 0): String {
        val dataJson = JSONObject().apply {
            put("event_type", change.data.event_type)
            put("category", change.data.category)
            put("source", change.data.source)
            put("source_id", change.data.source_id)
            put("summary", change.data.summary)
            put("occurred_at", change.data.occurred_at)
            put("data", JSONObject(change.data.data.toString()))
        }
        return JSONObject().apply {
            put("type", "change")
            put("event_id", change.event_id)
            put("change_type", change.change_type)
            put("data", dataJson)
            put("origin_device", deviceId)
            put("origin_seq", seq)
            put("hlc", "${System.currentTimeMillis()}-0-$deviceId")
            put("hop_path", JSONArray(listOf(deviceId)))
        }.toString()
    }
}
