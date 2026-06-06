package com.kazui.delphi

import android.os.Build
import android.os.Bundle
import android.provider.Settings
import android.util.Log
import android.view.WindowManager
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.lifecycle.lifecycleScope
import com.kazui.delphi.data.space.SpaceManager
import com.kazui.delphi.data.sync.PeerManager
import com.kazui.delphi.di.DatabaseProvider
import com.kazui.delphi.ui.navigation.DelphiNavGraph
import com.kazui.delphi.ui.theme.DelphiTheme
import dagger.hilt.android.AndroidEntryPoint
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import javax.inject.Inject

private const val TAG = "MainActivity"
private val DEVICE_ID_KEY = stringPreferencesKey("ark_device_id")

@AndroidEntryPoint
class MainActivity : ComponentActivity() {

    @Inject lateinit var peerManager: PeerManager
    @Inject lateinit var spaceManager: SpaceManager
    @Inject lateinit var databaseProvider: DatabaseProvider
    @Inject lateinit var dataStore: DataStore<Preferences>

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        requestHighRefreshRate()

        // Observe active space code and start/stop PeerManager accordingly
        var lastSpaceCode: String? = null
        lifecycleScope.launch {
            spaceManager.activeSpaceCode.collect { spaceCode ->
                if (spaceCode == lastSpaceCode) return@collect
                lastSpaceCode = spaceCode
                if (spaceCode != null) {
                    val deviceId = getOrCreateDeviceId()
                    val deviceName = getDeviceName()

                    // Start PeerManager on IO dispatcher — startSync is a blocking JNI call
                    // that must NOT run on the main thread (triggers ANR after 5s).
                    peerManager.onDataChanged = { }
                    withContext(Dispatchers.IO) {
                        peerManager.start(spaceCode, deviceId, deviceName)
                    }
                } else {
                    peerManager.stop()
                }
            }
        }

        setContent {
            DelphiTheme {
                DelphiNavGraph()
            }
        }
    }

    override fun onDestroy() {
        super.onDestroy()
        peerManager.stop()
    }

    private suspend fun getOrCreateDeviceId(): String {
        val prefs = dataStore.data.first()
        return prefs[DEVICE_ID_KEY] ?: run {
            val androidId = Settings.Secure.getString(contentResolver, Settings.Secure.ANDROID_ID)
            val newId = if (!androidId.isNullOrBlank() && androidId != "9774d56d682e549c") {
                "delphi-android-$androidId"
            } else {
                "delphi-android-${java.util.UUID.randomUUID()}"
            }
            dataStore.edit { it[DEVICE_ID_KEY] = newId }
            newId
        }
    }

    private fun getDeviceName(): String {
        return "${Build.MANUFACTURER} ${Build.MODEL}"
    }

    private fun requestHighRefreshRate() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            val display = display ?: return
            val highestMode = display.supportedModes.maxByOrNull { it.refreshRate } ?: return
            window.attributes = window.attributes.apply {
                preferredDisplayModeId = highestMode.modeId
            }
        } else {
            @Suppress("DEPRECATION")
            window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        }
    }
}
