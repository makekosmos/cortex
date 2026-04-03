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
            val todoDao = databaseProvider.todoDao()
            // Gather IDs first so we can cascade-delete related rows
            val trashedIds = todoDao.getTrashedIds()
            if (trashedIds.isNotEmpty()) {
                todoDao.deleteChecklistItemsByTodoIds(trashedIds)
                todoDao.deleteTagRefsByTodoIds(trashedIds)
            }
            todoDao.deleteTrashed()
        }
    }
}
