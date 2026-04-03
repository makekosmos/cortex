package com.kazui.delphi.ui.screens.inbox

import com.kazui.delphi.data.model.SmartList
import com.kazui.delphi.data.sync.ArkSyncClient
import com.kazui.delphi.data.sync.PeerManager
import com.kazui.delphi.di.DatabaseProvider
import com.kazui.delphi.ui.screens.SmartListViewModel
import dagger.hilt.android.lifecycle.HiltViewModel
import javax.inject.Inject

@HiltViewModel
class InboxViewModel @Inject constructor(
    databaseProvider: DatabaseProvider,
    arkSyncClient: ArkSyncClient,
    peerManager: PeerManager,
) : SmartListViewModel(databaseProvider, arkSyncClient, peerManager, SmartList.INBOX)
