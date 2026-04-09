package com.kazui.delphi.ui.screens.settings

import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.kazui.delphi.data.space.SpaceManager
import com.kazui.delphi.data.sync.ArkPeerManager
import com.kazui.delphi.data.sync.ArkSyncClient
import com.kazui.delphi.data.sync.LanSyncState
import com.kazui.delphi.data.sync.PeerManager
import com.kazui.delphi.data.sync.SyncStatus
import com.kazui.delphi.data.sync.parseConnectionString
import com.kazui.delphi.di.DatabaseProvider
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class SettingsViewModel @Inject constructor(
    private val peerManager: PeerManager,
    private val spaceManager: SpaceManager,
    private val databaseProvider: DatabaseProvider,
    private val dataStore: DataStore<Preferences>,
    // Legacy — kept for backward compat
    private val arkSyncClient: ArkSyncClient,
    private val arkPeerManager: ArkPeerManager,
) : ViewModel() {

    val activeSpaceCode: StateFlow<String?> = spaceManager.activeSpaceCode
        .stateIn(viewModelScope, SharingStarted.Eagerly, null)

    // PeerManager state
    val lanSyncState: StateFlow<LanSyncState> = peerManager.lanSyncState
        .stateIn(viewModelScope, SharingStarted.Eagerly, LanSyncState.DISCONNECTED)

    val connectedPeerCount: StateFlow<Int> = peerManager.connectedPeerCount
    val connectedPeerNames: StateFlow<List<String>> = peerManager.connectedPeerNames

    // Legacy Ark relay
    val syncStatus: StateFlow<SyncStatus> = arkSyncClient.status

    fun leaveSpace() {
        viewModelScope.launch {
            peerManager.stop()
            databaseProvider.close()
            spaceManager.clearActiveSpaceCode()
        }
    }

    fun formatSpaceCode(code: String): String = spaceManager.formatCode(code)

    fun getQrPayload(): String {
        val code = activeSpaceCode.value ?: return ""
        return spaceManager.generateQrPayload(code)
    }

    // Legacy methods preserved for hidden section
    fun connect(input: String) {
        val conn = parseConnectionString(input) ?: return
        viewModelScope.launch {
            val urlKey = stringPreferencesKey("ark_url")
            val keyKey = stringPreferencesKey("ark_api_key")
            dataStore.edit { prefs ->
                prefs[urlKey] = conn.serverUrl
                prefs[keyKey] = conn.apiKey
            }
            arkSyncClient.connect(conn.serverUrl, conn.apiKey)
        }
    }

    fun disconnect() {
        viewModelScope.launch {
            val urlKey = stringPreferencesKey("ark_url")
            val keyKey = stringPreferencesKey("ark_api_key")
            dataStore.edit { prefs ->
                prefs.remove(urlKey)
                prefs.remove(keyKey)
            }
            arkSyncClient.disconnect()
        }
    }

    fun resetSync() {
        arkSyncClient.resetAndResync()
    }
}
