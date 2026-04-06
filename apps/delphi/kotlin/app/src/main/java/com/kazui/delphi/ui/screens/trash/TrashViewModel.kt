package com.kazui.delphi.ui.screens.trash

import androidx.lifecycle.viewModelScope
import com.kazui.delphi.data.model.SmartList
import com.kazui.delphi.data.sync.ArkSyncClient
import com.kazui.delphi.data.sync.PeerManager
import com.kazui.delphi.di.DatabaseProvider
import com.kazui.delphi.ui.screens.SmartListViewModel
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class TrashViewModel @Inject constructor(
    databaseProvider: DatabaseProvider,
    arkSyncClient: ArkSyncClient,
    peerManager: PeerManager,
) : SmartListViewModel(databaseProvider, arkSyncClient, peerManager, SmartList.TRASH) {

    /** Permanently delete all trashed todos and their related checklist items / tag cross-refs. */
    fun emptyTrash() {
        viewModelScope.launch {
            val repo = databaseProvider.arkDataRepository
            val trashedIds = repo.getTrashedIds()
            if (trashedIds.isNotEmpty()) {
                // Broadcast hard-delete to all peers BEFORE local deletion
                trashedIds.forEach { id ->
                    peerManager.broadcastTodoDelete(id)
                }
                repo.deleteChecklistItemsByTodoIds(trashedIds)
                repo.deleteTagRefsByTodoIds(trashedIds)
            }
            repo.deleteTrashed()
        }
    }
}
