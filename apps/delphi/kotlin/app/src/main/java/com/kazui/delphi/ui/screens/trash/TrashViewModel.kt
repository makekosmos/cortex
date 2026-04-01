package com.kazui.delphi.ui.screens.trash

import com.kazui.delphi.data.db.TodoDao
import com.kazui.delphi.data.model.SmartList
import com.kazui.delphi.data.sync.ArkSyncClient
import com.kazui.delphi.data.sync.PeerManager
import com.kazui.delphi.ui.screens.SmartListViewModel
import dagger.hilt.android.lifecycle.HiltViewModel
import javax.inject.Inject

@HiltViewModel
class TrashViewModel @Inject constructor(
    todoDao: TodoDao,
    arkSyncClient: ArkSyncClient,
    peerManager: PeerManager,
) : SmartListViewModel(todoDao, arkSyncClient, peerManager, SmartList.TRASH)
