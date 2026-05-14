package com.kazui.delphi.data.sync

import java.time.Instant

data class HLC(
    val wallTime: String,
    val counter: Int,
    val deviceId: String,
) {
    companion object {
        fun now(deviceId: String): HLC {
            return HLC(Instant.now().toString(), 0, deviceId)
        }

        fun fromString(s: String): HLC {
            // Format: "2025-03-29T10:00:00.000Z:000001:device-id"
            // wallTime may contain ":", so take last 2 parts as counter and deviceId
            val parts = s.split(":")
            val deviceId = parts.last()
            val counter = parts[parts.size - 2].toInt()
            val wallTime = parts.dropLast(2).joinToString(":")
            return HLC(wallTime, counter, deviceId)
        }

        fun compare(a: HLC, b: HLC): Int {
            val timeCompare = a.wallTime.compareTo(b.wallTime)
            if (timeCompare != 0) return timeCompare
            val counterCompare = a.counter.compareTo(b.counter)
            if (counterCompare != 0) return counterCompare
            return a.deviceId.compareTo(b.deviceId)
        }
    }

    fun tick(): HLC {
        val now = Instant.now().toString()
        return if (now > wallTime) HLC(now, 0, deviceId)
        else HLC(wallTime, counter + 1, deviceId)
    }

    fun merge(remote: HLC): HLC {
        val now = Instant.now().toString()
        val maxTime = maxOf(wallTime, remote.wallTime, now)
        val newCounter = when (maxTime) {
            wallTime -> if (wallTime == remote.wallTime) maxOf(counter, remote.counter) + 1 else counter + 1
            remote.wallTime -> remote.counter + 1
            else -> 0
        }
        return HLC(maxTime, newCounter, deviceId)
    }

    override fun toString(): String {
        val paddedCounter = counter.toString().padStart(6, '0')
        return "$wallTime:$paddedCounter:$deviceId"
    }
}
