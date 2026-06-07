package com.kazui.delphi.ui.screens.space

import android.content.Context
import android.content.SharedPreferences
import android.util.Log
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.kazui.delphi.data.space.SpaceManager
import com.kazui.delphi.data.sync.PeerManager
import com.kazui.delphi.di.DatabaseProvider
import dagger.hilt.android.lifecycle.HiltViewModel
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import javax.inject.Inject

private const val TAG = "SpaceSetupViewModel"
private const val BACKUP_PREFS_NAME = "ark_space_backup"
private const val BACKUP_CODE_KEY = "active_space_code"

@HiltViewModel
class SpaceSetupViewModel @Inject constructor(
    private val spaceManager: SpaceManager,
    private val peerManager: PeerManager,
    private val databaseProvider: DatabaseProvider,
    private val dataStore: DataStore<Preferences>,
    @ApplicationContext private val context: Context,
) : ViewModel() {

    /** SharedPreferences fallback for space code persistence across DataStore failures. */
    private val backupPrefs: SharedPreferences =
        context.getSharedPreferences(BACKUP_PREFS_NAME, Context.MODE_PRIVATE)

    val activeSpaceCode: StateFlow<String?> = spaceManager.activeSpaceCode
        .stateIn(viewModelScope, SharingStarted.Eagerly, null)

    private val _isInitialized = MutableStateFlow(false)
    val isInitialized: StateFlow<Boolean> = _isInitialized.asStateFlow()

    val isArkDataAvailable: Boolean get() = databaseProvider.isArkDataAvailable

    private val _savedSpaces = MutableStateFlow<List<SpaceManager.SavedSpace>>(emptyList())
    val savedSpaces: StateFlow<List<SpaceManager.SavedSpace>> = _savedSpaces.asStateFlow()

    init {
        viewModelScope.launch {
            spaceManager.activeSpaceCode.collect { code ->
                if (!_isInitialized.value) {
                    // On first emission, if DataStore returned null, check SharedPreferences fallback
                    if (code == null) {
                        val fallbackCode = backupPrefs.getString(BACKUP_CODE_KEY, null)
                        if (!fallbackCode.isNullOrBlank()) {
                            Log.w(TAG, "DataStore returned null but SharedPreferences has code, restoring: $fallbackCode")
                            spaceManager.setActiveSpaceCode(fallbackCode)
                            // Don't set initialized yet -- the restored code will trigger another collect
                            return@collect
                        }
                    }
                    _isInitialized.value = true
                }
                // Keep SharedPreferences in sync as a backup
                if (code != null) {
                    backupPrefs.edit().putString(BACKUP_CODE_KEY, code).apply()
                    val spaceId = spaceManager.deriveSpaceId(code)
                    databaseProvider.switchTo(spaceId)
                } else {
                    backupPrefs.edit().remove(BACKUP_CODE_KEY).apply()
                }
            }
        }
        loadSavedSpaces()
    }

    private fun loadSavedSpaces() {
        viewModelScope.launch {
            _savedSpaces.value = spaceManager.getSavedSpaces()
        }
    }

    /** Validate user-entered code. */
    fun isValidCode(code: String): Boolean = spaceManager.isValidCode(code)

    /** Format raw code for display. */
    fun formatCode(code: String): String = spaceManager.formatCode(code)

    /** Generate a new random 12-char space code. */
    fun generateCode(): String = spaceManager.generateSpaceCode()

    /** Generate QR payload for the given code. */
    fun generateQrPayload(code: String): String = spaceManager.generateQrPayload(code)

    /** Parse a QR payload. Returns (normalizedCode, addresses) or null. */
    fun parseQrPayload(payload: String): Pair<String, List<String>>? = spaceManager.parseQrPayload(payload)

    /** Generate a 19-char extended code with embedded LAN IPv4. Returns null if no LAN IP. */
    fun generateExtendedCode(code: String): String? = spaceManager.generateExtendedCode(code)

    /** Parse a 19-char extended code. Returns (secret, addresses) or null. */
    fun parseExtendedCode(code: String): Pair<String, List<String>>? = spaceManager.parseExtendedCode(code)

    /**
     * Create a new space: save code, switch DB, start PeerManager (server + clients).
     */
    fun createSpace(code: String) {
        val normalized = spaceManager.normalizeCode(code)
        viewModelScope.launch {
            spaceManager.setActiveSpaceCode(normalized)
            val spaceId = spaceManager.deriveSpaceId(normalized)
            databaseProvider.switchTo(spaceId)

            val deviceId = getDeviceId()
            val deviceName = getDeviceName()
            withContext(Dispatchers.IO) {
                peerManager.start(normalized, deviceId, deviceName)
            }
        }
    }

    /**
     * Join a space: save code, switch DB, add initial peer addresses, start PeerManager.
     */
    fun joinSpace(code: String, addresses: List<String>) {
        val normalized = spaceManager.normalizeCode(code)
        viewModelScope.launch {
            spaceManager.setActiveSpaceCode(normalized)
            val spaceId = spaceManager.deriveSpaceId(normalized)
            databaseProvider.switchTo(spaceId)

            val deviceId = getDeviceId()
            val deviceName = getDeviceName()
            peerManager.start(normalized, deviceId, deviceName)

            // Connect to initial peer addresses from QR
            if (addresses.isNotEmpty()) {
                peerManager.addInitialPeer(addresses)
            }
        }
    }

    /** Leave the current space. */
    fun leaveSpace() {
        viewModelScope.launch {
            peerManager.stop()
            databaseProvider.close()
            spaceManager.clearActiveSpaceCode()
        }
    }

    /** Rejoin a previously saved space. */
    fun rejoinSpace(code: String) {
        val normalized = spaceManager.normalizeCode(code)
        viewModelScope.launch {
            spaceManager.setActiveSpaceCode(normalized)
            val spaceId = spaceManager.deriveSpaceId(normalized)
            databaseProvider.switchTo(spaceId)

            val deviceId = getDeviceId()
            val deviceName = getDeviceName()
            withContext(Dispatchers.IO) {
                peerManager.start(normalized, deviceId, deviceName)
            }
        }
    }

    /** Rename a saved space. */
    fun renameSpace(code: String, newName: String) {
        viewModelScope.launch {
            spaceManager.renameSpace(code, newName)
            _savedSpaces.value = spaceManager.getSavedSpaces()
        }
    }

    /** Delete a saved space and its sync data. */
    fun deleteSpace(code: String) {
        viewModelScope.launch {
            val normalized = spaceManager.normalizeCode(code)
            // If active, leave first
            val currentCode = spaceManager.activeSpaceCode.first()
            if (currentCode == normalized) {
                peerManager.stop()
                databaseProvider.close()
                spaceManager.clearActiveSpaceCode()
            }
            // Delete per-space DB files
            val spaceId = spaceManager.deriveSpaceId(normalized)
            databaseProvider.deleteSpaceDb(spaceId)

            spaceManager.removeSpaceFromList(normalized)
            _savedSpaces.value = spaceManager.getSavedSpaces()
        }
    }

    private suspend fun getDeviceId(): String {
        return dataStore.data.first()[stringPreferencesKey("ark_device_id")]
            ?: "delphi-android-unknown"
    }

    private fun getDeviceName(): String {
        return "${android.os.Build.MANUFACTURER} ${android.os.Build.MODEL}"
    }
}
