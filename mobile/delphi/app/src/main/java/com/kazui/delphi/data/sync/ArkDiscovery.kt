package com.kazui.delphi.data.sync

import android.content.Context
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import android.util.Log
import javax.inject.Inject
import javax.inject.Singleton

private const val TAG = "ArkDiscovery"
private const val SERVICE_TYPE = "_ark-sync._tcp"

@Singleton
class ArkDiscovery @Inject constructor() {

    private var nsdManager: NsdManager? = null
    private var discoveryListener: NsdManager.DiscoveryListener? = null

    private var onFoundCallback: ((String) -> Unit)? = null
    private var onLostCallback: (() -> Unit)? = null

    // Track the first found service to prefer it over later ones
    @Volatile private var resolvedServiceName: String? = null
    @Volatile private var isDiscoveryActive = false

    fun start(
        context: Context,
        onFound: (url: String) -> Unit,
        onLost: () -> Unit,
    ) {
        if (isDiscoveryActive) {
            Log.d(TAG, "Discovery already active, ignoring start()")
            return
        }

        onFoundCallback = onFound
        onLostCallback = onLost

        nsdManager = context.getSystemService(Context.NSD_SERVICE) as NsdManager

        discoveryListener = object : NsdManager.DiscoveryListener {
            override fun onStartDiscoveryFailed(serviceType: String, errorCode: Int) {
                Log.e(TAG, "onStartDiscoveryFailed: errorCode=$errorCode")
                isDiscoveryActive = false
            }

            override fun onStopDiscoveryFailed(serviceType: String, errorCode: Int) {
                Log.e(TAG, "onStopDiscoveryFailed: errorCode=$errorCode")
            }

            override fun onDiscoveryStarted(serviceType: String) {
                Log.d(TAG, "Discovery started for $serviceType")
                isDiscoveryActive = true
            }

            override fun onDiscoveryStopped(serviceType: String) {
                Log.d(TAG, "Discovery stopped for $serviceType")
                isDiscoveryActive = false
            }

            override fun onServiceFound(serviceInfo: NsdServiceInfo) {
                Log.d(TAG, "Service found: ${serviceInfo.serviceName} type=${serviceInfo.serviceType}")

                // Prefer first found: if we already resolved a service, skip new ones
                if (resolvedServiceName != null) {
                    Log.d(TAG, "Already tracking ${resolvedServiceName}, ignoring ${serviceInfo.serviceName}")
                    return
                }

                resolveService(serviceInfo)
            }

            override fun onServiceLost(serviceInfo: NsdServiceInfo) {
                Log.d(TAG, "Service lost: ${serviceInfo.serviceName}")

                if (serviceInfo.serviceName == resolvedServiceName) {
                    Log.d(TAG, "Active Ark server lost")
                    resolvedServiceName = null
                    onLostCallback?.invoke()
                }
            }
        }

        nsdManager?.discoverServices(SERVICE_TYPE, NsdManager.PROTOCOL_DNS_SD, discoveryListener)
    }

    private fun resolveService(serviceInfo: NsdServiceInfo) {
        // Android requires a fresh ResolveListener instance for each resolve call
        val resolveListener = object : NsdManager.ResolveListener {
            override fun onResolveFailed(failedServiceInfo: NsdServiceInfo, errorCode: Int) {
                Log.e(TAG, "Resolve failed for ${failedServiceInfo.serviceName}: errorCode=$errorCode")
                // Allow future services to be resolved if this one failed
                if (resolvedServiceName == null) {
                    // errorCode 3 = FAILURE_ALREADY_ACTIVE — retry once with a delay
                    if (errorCode == NsdManager.FAILURE_ALREADY_ACTIVE) {
                        Log.d(TAG, "Resolver busy (FAILURE_ALREADY_ACTIVE), will retry on next service found")
                    }
                }
            }

            override fun onServiceResolved(resolvedInfo: NsdServiceInfo) {
                Log.d(TAG, "Resolved: ${resolvedInfo.serviceName} host=${resolvedInfo.host} port=${resolvedInfo.port}")

                val host = resolvedInfo.host?.hostAddress
                val port = resolvedInfo.port

                if (host.isNullOrBlank() || port <= 0) {
                    Log.w(TAG, "Invalid resolved address: host=$host port=$port")
                    return
                }

                // Only accept the first successfully resolved service
                if (resolvedServiceName != null && resolvedServiceName != resolvedInfo.serviceName) {
                    Log.d(TAG, "Already tracking ${resolvedServiceName}, discarding ${resolvedInfo.serviceName}")
                    return
                }

                resolvedServiceName = resolvedInfo.serviceName
                val url = "http://$host:$port"
                Log.i(TAG, "Ark server discovered at $url")
                onFoundCallback?.invoke(url)
            }
        }

        try {
            nsdManager?.resolveService(serviceInfo, resolveListener)
        } catch (e: IllegalArgumentException) {
            Log.e(TAG, "resolveService threw: ${e.message}")
        }
    }

    fun stop() {
        val listener = discoveryListener
        if (listener != null && isDiscoveryActive) {
            try {
                nsdManager?.stopServiceDiscovery(listener)
            } catch (e: IllegalArgumentException) {
                Log.e(TAG, "stopServiceDiscovery threw: ${e.message}")
            }
        }
        discoveryListener = null
        nsdManager = null
        resolvedServiceName = null
        isDiscoveryActive = false
        onFoundCallback = null
        onLostCallback = null
    }
}
