package com.kazui.delphi.data.sync

data class ArkConnection(val serverUrl: String, val apiKey: String)

fun parseConnectionString(input: String): ArkConnection? {
    return try {
        // Format: ark://host:port?key=SECRET
        val stripped = input.trim()
        if (!stripped.startsWith("ark://")) return null
        val withoutScheme = stripped.removePrefix("ark://")
        val queryIndex = withoutScheme.indexOf('?')
        if (queryIndex == -1) return null
        val hostPort = withoutScheme.substring(0, queryIndex)
        val query = withoutScheme.substring(queryIndex + 1)
        val key = query.split("&")
            .firstOrNull { it.startsWith("key=") }
            ?.removePrefix("key=")
            ?: return null
        ArkConnection("http://$hostPort", key)
    } catch (_: Exception) {
        null
    }
}
