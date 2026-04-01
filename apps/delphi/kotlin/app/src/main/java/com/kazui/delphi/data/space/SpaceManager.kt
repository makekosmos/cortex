package com.kazui.delphi.data.space

import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map
import java.net.Inet4Address
import java.net.Inet6Address
import java.net.NetworkInterface
import java.security.MessageDigest
import java.security.SecureRandom
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Manages Ark Space -- the shared P2P mesh secret.
 *
 * v3 (equal peers): 12-character random Base32-Crockford code, format XXXX-XXXX-XXXX.
 *   Every device is both WS server and WS client.
 *   Legacy 7-char codes still accepted for backward compat.
 */
@Singleton
class SpaceManager @Inject constructor(
    private val dataStore: DataStore<Preferences>,
) {
    companion object {
        private val ACTIVE_SPACE_KEY = stringPreferencesKey("ark.space.activeCode")
        // Base32-Crockford alphabet: no I, L, O, U
        const val ALPHABET = "0123456789ABCDEFGHJKMNPQRSTVWXYZ"
        const val LAN_SYNC_PORT = 21531
    }

    // ---------------------------------------------------------------------------
    // Persistence
    // ---------------------------------------------------------------------------

    /** Flow emitting the active space code (raw, no dashes), or null if none. */
    val activeSpaceCode: Flow<String?> = dataStore.data.map { it[ACTIVE_SPACE_KEY] }

    suspend fun setActiveSpaceCode(code: String) {
        dataStore.edit { it[ACTIVE_SPACE_KEY] = normalizeCode(code) }
    }

    suspend fun clearActiveSpaceCode() {
        dataStore.edit { it.remove(ACTIVE_SPACE_KEY) }
    }

    // ---------------------------------------------------------------------------
    // Code generation
    // ---------------------------------------------------------------------------

    /** Generate a random 12-character Base32-Crockford code. */
    fun generateSpaceCode(): String {
        val random = SecureRandom()
        val bytes = ByteArray(12)
        random.nextBytes(bytes)
        return bytes.joinToString("") { ALPHABET[(it.toInt() and 0xFF) % 32].toString() }
    }

    // ---------------------------------------------------------------------------
    // Code normalization & formatting
    // ---------------------------------------------------------------------------

    /** Remove dashes/spaces and uppercase -- returns the raw code (7 or 12 chars). */
    fun normalizeCode(code: String): String =
        code.replace("-", "").replace(" ", "").uppercase()

    /** Format code with dashes -- handles both 7-char (XXXX-XXX) and 12-char (XXXX-XXXX-XXXX). */
    fun formatCode(code: String): String {
        val raw = normalizeCode(code)
        return when (raw.length) {
            7 -> "${raw.substring(0, 4)}-${raw.substring(4, 7)}"
            12 -> "${raw.substring(0, 4)}-${raw.substring(4, 8)}-${raw.substring(8, 12)}"
            else -> code
        }
    }

    /** Validate that a code is valid Base32-Crockford (7-char legacy or 12-char). */
    fun isValidCode(code: String): Boolean {
        val raw = normalizeCode(code)
        if (raw.length != 7 && raw.length != 12) return false
        return raw.all { it in ALPHABET }
    }

    /**
     * Derive a stable space ID: SHA-256(normalized code) -> first 16 hex chars.
     * Used for peer validation during sync handshake.
     */
    fun deriveSpaceId(code: String): String {
        val raw = normalizeCode(code)
        val digest = MessageDigest.getInstance("SHA-256")
        val hash = digest.digest(raw.toByteArray(Charsets.UTF_8))
        return hash.joinToString("") { "%02x".format(it) }.take(16)
    }

    // ---------------------------------------------------------------------------
    // Network addresses
    // ---------------------------------------------------------------------------

    /**
     * Collect all non-loopback IP addresses for this device.
     * Returns addresses formatted as "IP:port" (e.g. "192.168.1.151:21531").
     */
    fun getOwnAddresses(port: Int = LAN_SYNC_PORT): List<String> {
        val addresses = mutableListOf<String>()
        try {
            val interfaces = NetworkInterface.getNetworkInterfaces() ?: return addresses
            for (iface in interfaces) {
                if (iface.isLoopback || !iface.isUp) continue
                for (addr in iface.inetAddresses) {
                    if (addr.isLoopbackAddress) continue
                    when (addr) {
                        is Inet4Address -> {
                            addresses.add("${addr.hostAddress}:$port")
                        }
                        is Inet6Address -> {
                            // Include link-local with scope ID
                            val host = addr.hostAddress ?: continue
                            // Remove any %scope suffix for bracket formatting, re-add
                            val cleanHost = host.substringBefore('%')
                            val scope = if (host.contains('%')) "%${host.substringAfter('%')}" else ""
                            addresses.add("[${cleanHost}${scope}]:$port")
                        }
                    }
                }
            }
        } catch (_: Exception) {
            // NetworkInterface access can fail on some devices
        }
        return addresses
    }

    /**
     * Build a QR payload for sharing the space.
     * Format: ark://join?code=XXXX-XXXX-XXXX&addrs=ip1:port,ip2:port
     */
    fun generateQrPayload(code: String, addresses: List<String> = getOwnAddresses()): String {
        val formatted = formatCode(code)
        val addrs = addresses.joinToString(",")
        return "ark://join?code=$formatted&addrs=$addrs"
    }

    /**
     * Parse an ark://join?... QR payload.
     * Returns (normalizedCode, addresses) or null if invalid.
     */
    fun parseQrPayload(payload: String): Pair<String, List<String>>? {
        if (!payload.startsWith("ark://join?")) return null
        val query = payload.substringAfter("ark://join?")
        val params = query.split("&").associate { part ->
            val (key, value) = part.split("=", limit = 2).let {
                if (it.size == 2) it[0] to it[1] else it[0] to ""
            }
            key to value
        }
        val code = params["code"] ?: return null
        val normalized = normalizeCode(code)
        if (!isValidCode(normalized)) return null
        val addresses = params["addrs"]?.split(",")?.filter { it.isNotBlank() } ?: emptyList()
        return normalized to addresses
    }
}
