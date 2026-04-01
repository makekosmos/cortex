package com.kazui.delphi.data.sync

import android.util.Log
import java.security.SecureRandom
import javax.crypto.Mac
import javax.crypto.spec.SecretKeySpec

private const val TAG = "ArkPeerProtocol"

object ArkPeerProtocol {

    fun computeMeshId(meshSecret: String): String {
        val mac = Mac.getInstance("HmacSHA256")
        mac.init(SecretKeySpec("mesh-id".toByteArray(Charsets.UTF_8), "HmacSHA256"))
        mac.update(meshSecret.toByteArray(Charsets.UTF_8))
        return mac.doFinal().toHex().take(16)
    }

    fun computeAuthHmac(meshSecret: String, nonce: String): String {
        val mac = Mac.getInstance("HmacSHA256")
        mac.init(SecretKeySpec(meshSecret.toByteArray(Charsets.UTF_8), "HmacSHA256"))
        mac.update(nonce.toByteArray(Charsets.UTF_8))
        return mac.doFinal().toHex()
    }

    fun verifyAuthHmac(meshSecret: String, nonce: String, provided: String): Boolean {
        return try {
            val expected = computeAuthHmac(meshSecret, nonce)
            expected.equals(provided, ignoreCase = true)
        } catch (e: Exception) {
            Log.e(TAG, "HMAC verification error: ${e.message}")
            false
        }
    }

    fun generateNonce(): String {
        val bytes = ByteArray(32)
        SecureRandom().nextBytes(bytes)
        return bytes.toHex()
    }

    private fun ByteArray.toHex() = joinToString("") { "%02x".format(it) }
}
