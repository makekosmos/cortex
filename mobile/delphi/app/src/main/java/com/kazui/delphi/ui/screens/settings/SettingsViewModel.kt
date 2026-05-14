package com.kazui.delphi.ui.screens.settings

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.kazui.delphi.data.space.SpaceManager
import com.kazui.delphi.data.sync.LanSyncState
import com.kazui.delphi.data.sync.PeerManager
import com.kazui.delphi.di.DatabaseProvider
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class SettingsViewModel @Inject constructor(
    private val peerManager: PeerManager,
    private val spaceManager: SpaceManager,
    private val databaseProvider: DatabaseProvider,
) : ViewModel() {

    val activeSpaceCode: StateFlow<String?> = spaceManager.activeSpaceCode
        .stateIn(viewModelScope, SharingStarted.Eagerly, null)

    // PeerManager state
    val lanSyncState: StateFlow<LanSyncState> = peerManager.lanSyncState
        .stateIn(viewModelScope, SharingStarted.Eagerly, LanSyncState.DISCONNECTED)

    val connectedPeerCount: StateFlow<Int> = peerManager.connectedPeerCount
    val connectedPeerNames: StateFlow<List<String>> = peerManager.connectedPeerNames

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
}
