package com.kazui.delphi.data.sync

import org.json.JSONArray
import org.json.JSONObject

/**
 * Multi-address peer record, Syncthing-style.
 * Each known peer stores all discovered addresses.
 */
data class PeerRecord(
    val deviceId: String,
    val deviceName: String,
    val addresses: List<String>,
    val lastSeen: String,
    val lastAddress: String? = null,
) {
    fun toJson(): JSONObject = JSONObject().apply {
        put("device_id", deviceId)
        put("device_name", deviceName)
        put("addresses", JSONArray(addresses))
        put("last_seen", lastSeen)
        lastAddress?.let { put("last_address", it) }
    }

    companion object {
        fun fromJson(json: JSONObject): PeerRecord {
            val addressesArr = json.optJSONArray("addresses")
            val addresses = if (addressesArr != null) {
                (0 until addressesArr.length()).map { addressesArr.getString(it) }
            } else {
                emptyList()
            }
            return PeerRecord(
                deviceId = json.optString("device_id", ""),
                deviceName = json.optString("device_name", ""),
                addresses = addresses,
                lastSeen = json.optString("last_seen", ""),
                lastAddress = json.optString("last_address", "").ifEmpty { null },
            )
        }
    }
}

/**
 * Merge incoming peer records into existing ones.
 * - Union merge of addresses (no duplicates)
 * - Preserve the newest last_seen
 * - Preserve last_address from whichever record is newer
 * - New peers are appended
 */
fun mergePeerRecords(
    existing: List<PeerRecord>,
    incoming: List<PeerRecord>,
): List<PeerRecord> {
    val map = mutableMapOf<String, PeerRecord>()

    for (peer in existing) {
        map[peer.deviceId] = peer.copy()
    }

    for (inc in incoming) {
        val current = map[inc.deviceId]
        if (current == null) {
            map[inc.deviceId] = inc.copy()
            continue
        }

        // Union merge addresses
        val addrSet = LinkedHashSet(current.addresses)
        addrSet.addAll(inc.addresses)

        // Keep newest last_seen
        var newName = current.deviceName
        var newLastAddr = current.lastAddress
        if (inc.lastSeen > current.lastSeen) {
            newName = inc.deviceName
            if (inc.lastAddress != null) {
                newLastAddr = inc.lastAddress
            }
        }

        map[inc.deviceId] = current.copy(
            addresses = addrSet.toList(),
            deviceName = newName,
            lastSeen = maxOf(current.lastSeen, inc.lastSeen),
            lastAddress = newLastAddr,
        )
    }

    return map.values.toList()
}
