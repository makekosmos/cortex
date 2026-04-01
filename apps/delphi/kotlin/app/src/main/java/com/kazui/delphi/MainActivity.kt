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
import com.kazui.delphi.data.db.ProjectDao
import com.kazui.delphi.data.db.TodoDao
import com.kazui.delphi.data.space.SpaceManager
import com.kazui.delphi.data.sync.ArkEventMapper
import com.kazui.delphi.data.sync.ArkPeerManager
import com.kazui.delphi.data.sync.PeerManager
import com.kazui.delphi.ui.navigation.DelphiNavGraph
import com.kazui.delphi.ui.theme.DelphiTheme
import dagger.hilt.android.AndroidEntryPoint
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import javax.inject.Inject

private const val TAG = "MainActivity"
private val DEVICE_ID_KEY = stringPreferencesKey("ark_device_id")

@AndroidEntryPoint
class MainActivity : ComponentActivity() {

    @Inject lateinit var arkPeerManager: ArkPeerManager
    @Inject lateinit var peerManager: PeerManager
    @Inject lateinit var spaceManager: SpaceManager
    @Inject lateinit var todoDao: TodoDao
    @Inject lateinit var projectDao: ProjectDao
    @Inject lateinit var dataStore: DataStore<Preferences>

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        requestHighRefreshRate()

        // Wire up the incoming-change handler for legacy P2P mesh
        arkPeerManager.onChangeReceived = { change ->
            lifecycleScope.launch {
                try {
                    when {
                        ArkEventMapper.isTaskChange(change) -> {
                            if (change.change_type == "delete") {
                                todoDao.deleteById(change.data.source_id)
                            } else {
                                ArkEventMapper.arkChangeToTodoItem(change)?.let { todoDao.upsert(it) }
                            }
                        }
                        ArkEventMapper.isProjectChange(change) -> {
                            if (change.change_type == "delete") {
                                projectDao.deleteProjectById(change.data.source_id)
                            } else {
                                ArkEventMapper.arkChangeToProject(change)?.let { projectDao.upsertProject(it) }
                            }
                        }
                    }
                } catch (e: Exception) {
                    Log.e(TAG, "Failed to apply incoming P2P change: ${e.message}")
                }
            }
        }

        // Observe active space code and start/stop services accordingly
        var lastSpaceCode: String? = null
        lifecycleScope.launch {
            spaceManager.activeSpaceCode.collect { spaceCode ->
                if (spaceCode == lastSpaceCode) return@collect
                lastSpaceCode = spaceCode
                if (spaceCode != null) {
                    val deviceId = getOrCreateDeviceId()
                    val deviceName = getDeviceName()

                    // Start legacy P2P mesh
                    arkPeerManager.start(
                        context = this@MainActivity,
                        meshSecret = spaceCode,
                        deviceId = deviceId,
                        deviceName = deviceName,
                    )

                    // Start PeerManager (WS server + client connections)
                    peerManager.onDataChanged = { }
                    peerManager.start(spaceCode, deviceId, deviceName)
                } else {
                    arkPeerManager.stop()
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
        arkPeerManager.stop()
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
