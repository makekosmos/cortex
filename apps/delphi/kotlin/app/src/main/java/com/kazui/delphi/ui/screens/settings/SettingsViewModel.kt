package com.kazui.delphi.ui.screens.settings

import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.kazui.delphi.data.sync.ArkSyncClient
import com.kazui.delphi.data.sync.SyncStatus
import com.kazui.delphi.data.sync.parseConnectionString
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class SettingsViewModel @Inject constructor(
    private val arkSyncClient: ArkSyncClient,
    private val dataStore: DataStore<Preferences>,
) : ViewModel() {

    private val ARK_URL_KEY = stringPreferencesKey("ark_url")
    private val ARK_KEY_KEY = stringPreferencesKey("ark_api_key")

    val syncStatus: StateFlow<SyncStatus> = arkSyncClient.status

    private val _savedUrl = MutableStateFlow<String?>(null)
    val savedUrl: StateFlow<String?> = _savedUrl.asStateFlow()

    init {
        viewModelScope.launch {
            dataStore.data.first().let { prefs ->
                _savedUrl.value = prefs[ARK_URL_KEY]
                val url = prefs[ARK_URL_KEY]
                val key = prefs[ARK_KEY_KEY]
                if (!url.isNullOrBlank() && !key.isNullOrBlank()) {
                    arkSyncClient.connect(url, key)
                }
            }
        }
    }

    fun connect(input: String) {
        val conn = parseConnectionString(input) ?: return
        viewModelScope.launch {
            dataStore.edit { prefs ->
                prefs[ARK_URL_KEY] = conn.serverUrl
                prefs[ARK_KEY_KEY] = conn.apiKey
            }
            _savedUrl.value = conn.serverUrl
            arkSyncClient.connect(conn.serverUrl, conn.apiKey)
        }
    }

    fun disconnect() {
        viewModelScope.launch {
            dataStore.edit { prefs ->
                prefs.remove(ARK_URL_KEY)
                prefs.remove(ARK_KEY_KEY)
            }
            _savedUrl.value = null
            arkSyncClient.disconnect()
        }
    }

    fun resetSync() {
        arkSyncClient.resetAndResync()
    }

    fun clearLocalData() {
        arkSyncClient.clearLocalData()
    }
}
