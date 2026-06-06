package com.kazui.delphi.data.space

import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import org.json.JSONArray
import org.json.JSONObject
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
    data class SavedSpace(
        val code: String,
        val name: String,
        val createdAt: String,
    )

    companion object {
        private val ACTIVE_SPACE_KEY = stringPreferencesKey("ark.space.activeCode")
        private val SAVED_SPACES_KEY = stringPreferencesKey("ark.space.savedSpaces")
        // Base32-Crockford alphabet: no I, L, O, U
        const val ALPHABET = "0123456789ABCDEFGHJKMNPQRSTVWXYZ"
        const val LAN_SYNC_PORT = 21531
        const val DEFAULT_SPACE_NAME = "Новое пространство"
    }

    // ---------------------------------------------------------------------------
    // Persistence
    // ---------------------------------------------------------------------------

    /** Flow emitting the active space code (raw, no dashes), or null if none. */
    val activeSpaceCode: Flow<String?> = dataStore.data.map { it[ACTIVE_SPACE_KEY] }

    suspend fun setActiveSpaceCode(code: String) {
        val normalized = normalizeCode(code)
        dataStore.edit { it[ACTIVE_SPACE_KEY] = normalized }
        // Also save to space list
        saveSpaceToList(normalized)
    }

    suspend fun clearActiveSpaceCode() {
        dataStore.edit { it.remove(ACTIVE_SPACE_KEY) }
    }

    // ---------------------------------------------------------------------------
    // Saved spaces list
    // ---------------------------------------------------------------------------

    /** Get all saved spaces from DataStore. */
    suspend fun getSavedSpaces(): List<SavedSpace> {
        return try {
            val prefs = dataStore.data.first()
            val raw = prefs[SAVED_SPACES_KEY] ?: return emptyList()
            val arr = JSONArray(raw)
            (0 until arr.length()).map { i ->
                val obj = arr.getJSONObject(i)
                SavedSpace(
                    code = obj.optString("code", ""),
                    name = obj.optString("name", ""),
                    createdAt = obj.optString("createdAt", ""),
                )
            }
        } catch (_: Exception) {
            emptyList()
        }
    }

    /** Save a space to the list (dedup by code). */
    private suspend fun saveSpaceToList(code: String) {
        val existing = getSavedSpaces()
        val prev = existing.find { it.code == code }
        val rest = existing.filter { it.code != code }.toMutableList()
        // Keep existing name if re-saving; otherwise default to "Новое пространство"
        val name = prev?.name ?: DEFAULT_SPACE_NAME
        rest.add(0, SavedSpace(code, name, prev?.createdAt ?: java.time.Instant.now().toString()))
        persistSpaceList(rest)
    }

    /** Rename a saved space (display-only label; code stays unchanged). */
    suspend fun renameSpace(code: String, newName: String) {
        val normalized = normalizeCode(code)
        val spaces = getSavedSpaces().map { space ->
            if (space.code == normalized) space.copy(name = newName.trim().ifEmpty { DEFAULT_SPACE_NAME })
            else space
        }
        persistSpaceList(spaces)
    }

    /** Remove a space from the saved list. */
    suspend fun removeSpaceFromList(code: String) {
        val spaces = getSavedSpaces().filter { it.code != code }
        persistSpaceList(spaces)
    }

    private suspend fun persistSpaceList(spaces: List<SavedSpace>) {
        val arr = JSONArray()
        for (s in spaces) {
            arr.put(JSONObject().apply {
                put("code", s.code)
                put("name", s.name)
                put("createdAt", s.createdAt)
            })
        }
        dataStore.edit { it[SAVED_SPACES_KEY] = arr.toString() }
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

    /** Format code with dashes -- handles 7-char, 12-char, and 19-char (XXXX-XXXX-XXXX-XXXX-XXX) extended codes. */
    fun formatCode(code: String): String {
        val raw = normalizeCode(code)
        return when (raw.length) {
            7 -> "${raw.substring(0, 4)}-${raw.substring(4, 7)}"
            12 -> "${raw.substring(0, 4)}-${raw.substring(4, 8)}-${raw.substring(8, 12)}"
            19 -> "${raw.substring(0, 4)}-${raw.substring(4, 8)}-${raw.substring(8, 12)}-${raw.substring(12, 16)}-${raw.substring(16, 19)}"
            else -> code
        }
    }

    // ---------------------------------------------------------------------------
    // Extended code (v4): 12-char secret + 7-char encoded IPv4
    // ---------------------------------------------------------------------------

    /** Encode an IPv4 string ("192.168.1.70") into 7 Base32-Crockford characters. */
    fun encodeIpv4(ipv4: String): String? {
        val parts = ipv4.split(".").mapNotNull { it.toIntOrNull() }
        if (parts.size != 4 || parts.any { it < 0 || it > 255 }) return null
        val n = (parts[0].toLong() shl 24) or (parts[1].toLong() shl 16) or
                (parts[2].toLong() shl 8) or parts[3].toLong()
        val shifted = n shl 3 // 32 bits -> 35 bits, 3 low padding bits = 0
        return buildString {
            for (i in 6 downTo 0) {
                append(ALPHABET[((shifted shr (i * 5)) and 0x1FL).toInt()])
            }
        }
    }

    /** Decode 7 Base32-Crockford characters back to an IPv4 string. */
    fun decodeIpv4(encoded: String): String? {
        val clean = encoded.uppercase()
        if (clean.length != 7 || clean.any { it !in ALPHABET }) return null
        var value = 0L
        for (c in clean) {
            value = (value shl 5) or ALPHABET.indexOf(c).toLong()
        }
        value = value shr 3 // remove 3 padding bits
        val a = ((value shr 24) and 0xFF).toInt()
        val b = ((value shr 16) and 0xFF).toInt()
        val c = ((value shr 8) and 0xFF).toInt()
        val d = (value and 0xFF).toInt()
        return "$a.$b.$c.$d"
    }

    /**
     * Build a 19-char extended code = 12-char secret + 7-char encoded primary LAN IPv4.
     * Returns null if no LAN IPv4 address is available.
     */
    fun generateExtendedCode(code: String): String? {
        val primaryAddr = getOwnAddresses()
            .firstOrNull { it.matches(Regex("""\d+\.\d+\.\d+\.\d+:\d+""")) }
            ?: return null
        val ipv4 = primaryAddr.substringBefore(":")
        val encoded = encodeIpv4(ipv4) ?: return null
        return normalizeCode(code).take(12) + encoded
    }

    /**
     * Parse a 19-char extended code: first 12 chars = secret, last 7 = encoded IPv4.
     * Returns Pair(secret, listOf("ip:port")) or null if invalid.
     */
    fun parseExtendedCode(code: String): Pair<String, List<String>>? {
        val clean = normalizeCode(code)
        if (clean.length != 19 || clean.any { it !in ALPHABET }) return null
        val secret = clean.take(12)
        val ipv4 = decodeIpv4(clean.drop(12)) ?: return null
        return secret to listOf("$ipv4:$LAN_SYNC_PORT")
    }

    /** Validate that a code is valid Base32-Crockford (7-char legacy, 12-char, or 19-char extended). */
    fun isValidCode(code: String): Boolean {
        val raw = normalizeCode(code)
        if (raw.length != 7 && raw.length != 12 && raw.length != 19) return false
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
