package com.kazui.delphi.data.sync

import android.util.Log
import kotlinx.coroutines.*
import org.json.JSONObject
import java.net.DatagramPacket
import java.net.DatagramSocket
import java.net.InetAddress
import java.net.InetSocketAddress
import java.net.NetworkInterface
import java.net.SocketTimeoutException

private const val TAG = "BroadcastDiscovery"
private const val BEACON_PORT = 21532  // LAN_SYNC_PORT + 1
private const val BEACON_INTERVAL_MS = 5_000L
private const val BEACON_TYPE = "delphi"
private const val RECV_BUFFER_SIZE = 1024

/**
 * UDP Broadcast-based peer discovery for LAN sync.
 *
 * Unlike mDNS (which uses multicast and is often blocked by routers),
 * UDP broadcast works on virtually all LANs. Each peer periodically
 * sends a beacon containing its space_id (hashed, safe to broadcast)
 * and WS server port. Listeners on the same space use the source IP
 * to connect directly.
 */
class BroadcastDiscovery(
    private val scope: CoroutineScope,
) {
    data class BeaconPeer(
        val deviceId: String,
        val deviceName: String,
        val address: String,  // "ip:wsPort"
    )

    private var sendSocket: DatagramSocket? = null
    private var recvSocket: DatagramSocket? = null
    private var sendJob: Job? = null
    private var recvJob: Job? = null
    @Volatile private var isRunning = false

    private var spaceId: String = ""
    private var deviceId: String = ""
    private var deviceName: String = ""
    private var wsPort: Int = 21531

    var onPeerDiscovered: ((BeaconPeer) -> Unit)? = null

    fun start(spaceId: String, deviceId: String, deviceName: String, wsPort: Int) {
        stop()
        this.spaceId = spaceId
        this.deviceId = deviceId
        this.deviceName = deviceName
        this.wsPort = wsPort
        this.isRunning = true

        // Receiver
        recvJob = scope.launch(Dispatchers.IO) {
            try {
                recvSocket = DatagramSocket(null).apply {
                    reuseAddress = true
                    bind(InetSocketAddress(BEACON_PORT))
                    soTimeout = 2000  // 2s timeout for interruptibility
                }
                Log.i(TAG, "Listening on UDP $BEACON_PORT")

                val buffer = ByteArray(RECV_BUFFER_SIZE)
                while (isRunning && isActive) {
                    try {
                        val packet = DatagramPacket(buffer, buffer.size)
                        recvSocket?.receive(packet)
                        val msg = String(packet.data, 0, packet.length)
                        handleMessage(msg, packet.address.hostAddress ?: "")
                    } catch (_: SocketTimeoutException) {
                        // expected, loop continues
                    } catch (e: Exception) {
                        if (isRunning) Log.w(TAG, "Receive error: ${e.message}")
                    }
                }
            } catch (e: Exception) {
                Log.w(TAG, "Failed to start receiver: ${e.message}")
            }
        }

        // Sender
        sendJob = scope.launch(Dispatchers.IO) {
            try {
                sendSocket = DatagramSocket().apply {
                    broadcast = true
                }

                while (isRunning && isActive) {
                    sendBeacon()
                    delay(BEACON_INTERVAL_MS)
                }
            } catch (e: Exception) {
                if (isRunning) Log.w(TAG, "Sender error: ${e.message}")
            }
        }
    }

    fun stop() {
        isRunning = false
        sendJob?.cancel()
        recvJob?.cancel()
        sendJob = null
        recvJob = null
        try { sendSocket?.close() } catch (_: Exception) {}
        try { recvSocket?.close() } catch (_: Exception) {}
        sendSocket = null
        recvSocket = null
    }

    private fun sendBeacon() {
        val beacon = JSONObject().apply {
            put("t", BEACON_TYPE)
            put("s", spaceId)
            put("d", deviceId)
            put("n", deviceName)
            put("p", wsPort)
        }.toString()

        val data = beacon.toByteArray()

        // Send to subnet broadcast addresses
        val broadcastAddrs = getBroadcastAddresses()
        for (addr in broadcastAddrs) {
            try {
                val packet = DatagramPacket(data, data.size, addr, BEACON_PORT)
                sendSocket?.send(packet)
            } catch (_: Exception) {
                // ignore per-interface errors
            }
        }
    }

    private fun handleMessage(msg: String, senderIp: String) {
        try {
            val data = JSONObject(msg)
            if (data.optString("t") != BEACON_TYPE) return
            if (data.optString("s") != spaceId) return  // different space
            if (data.optString("d") == deviceId) return  // from self

            val port = data.optInt("p", 21531)
            val peer = BeaconPeer(
                deviceId = data.optString("d"),
                deviceName = data.optString("n", "Unknown"),
                address = "$senderIp:$port",
            )

            onPeerDiscovered?.invoke(peer)
        } catch (_: Exception) {
            // ignore malformed messages
        }
    }

    private fun getBroadcastAddresses(): List<InetAddress> {
        val addresses = mutableListOf<InetAddress>()
        try {
            // Global broadcast
            addresses.add(InetAddress.getByName("255.255.255.255"))

            // Subnet broadcasts
            val interfaces = NetworkInterface.getNetworkInterfaces() ?: return addresses
            for (iface in interfaces) {
                if (iface.isLoopback || !iface.isUp) continue
                for (ifAddr in iface.interfaceAddresses) {
                    val broadcast = ifAddr.broadcast ?: continue
                    if (!addresses.contains(broadcast)) {
                        addresses.add(broadcast)
                    }
                }
            }
        } catch (_: Exception) {
            // fallback to global broadcast only
        }
        return addresses
    }
}
