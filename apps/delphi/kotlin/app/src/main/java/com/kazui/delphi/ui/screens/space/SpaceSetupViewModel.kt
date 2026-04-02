package com.kazui.delphi.ui.screens.space

import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.kazui.delphi.data.space.SpaceManager
import com.kazui.delphi.data.sync.PeerManager
import com.kazui.delphi.data.sync.PeerRecord
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class SpaceSetupViewModel @Inject constructor(
    private val spaceManager: SpaceManager,
    private val peerManager: PeerManager,
    private val dataStore: DataStore<Preferences>,
) : ViewModel() {

    val activeSpaceCode: StateFlow<String?> = spaceManager.activeSpaceCode
        .stateIn(viewModelScope, SharingStarted.Eagerly, null)

    private val _isInitialized = MutableStateFlow(false)
    val isInitialized: StateFlow<Boolean> = _isInitialized.asStateFlow()

    private val _savedSpaces = MutableStateFlow<List<SpaceManager.SavedSpace>>(emptyList())
    val savedSpaces: StateFlow<List<SpaceManager.SavedSpace>> = _savedSpaces.asStateFlow()

    init {
        viewModelScope.launch {
            spaceManager.activeSpaceCode.collect { _ ->
                if (!_isInitialized.value) _isInitialized.value = true
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
     * Create a new space: save code, start PeerManager (server + clients).
     */
    fun createSpace(code: String) {
        val normalized = spaceManager.normalizeCode(code)
        viewModelScope.launch {
            spaceManager.setActiveSpaceCode(normalized)

            val deviceId = getDeviceId()
            val deviceName = getDeviceName()
            peerManager.start(normalized, deviceId, deviceName)
        }
    }

    /**
     * Join a space: save code, add initial peer addresses, start PeerManager.
     */
    fun joinSpace(code: String, addresses: List<String>) {
        val normalized = spaceManager.normalizeCode(code)
        viewModelScope.launch {
            spaceManager.setActiveSpaceCode(normalized)

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
            spaceManager.clearActiveSpaceCode()
        }
    }

    /** Rejoin a previously saved space. */
    fun rejoinSpace(code: String) {
        val normalized = spaceManager.normalizeCode(code)
        viewModelScope.launch {
            spaceManager.setActiveSpaceCode(normalized)
            val deviceId = getDeviceId()
            val deviceName = getDeviceName()
            peerManager.start(normalized, deviceId, deviceName)
        }
    }

    /** Delete a saved space and its sync data. */
    fun deleteSpace(code: String) {
        viewModelScope.launch {
            // If active, leave first
            val currentCode = spaceManager.activeSpaceCode.first()
            if (currentCode == spaceManager.normalizeCode(code)) {
                peerManager.stop()
                spaceManager.clearActiveSpaceCode()
            }
            spaceManager.removeSpaceFromList(spaceManager.normalizeCode(code))
            // Clear sync data for this space (version vector, known peers are per-space via DataStore)
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
