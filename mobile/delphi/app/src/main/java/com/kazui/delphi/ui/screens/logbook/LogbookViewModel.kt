package com.kazui.delphi.ui.screens.logbook

import com.kazui.delphi.data.model.SmartList
import com.kazui.delphi.data.sync.PeerManager
import com.kazui.delphi.di.DatabaseProvider
import com.kazui.delphi.ui.screens.SmartListViewModel
import dagger.hilt.android.lifecycle.HiltViewModel
import javax.inject.Inject

@HiltViewModel
class LogbookViewModel @Inject constructor(
    databaseProvider: DatabaseProvider,
    peerManager: PeerManager,
) : SmartListViewModel(databaseProvider, peerManager, SmartList.LOGBOOK)
