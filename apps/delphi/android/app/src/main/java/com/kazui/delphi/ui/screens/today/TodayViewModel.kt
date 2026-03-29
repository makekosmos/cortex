package com.kazui.delphi.ui.screens.today

import com.kazui.delphi.data.db.TodoDao
import com.kazui.delphi.data.model.SmartList
import com.kazui.delphi.data.sync.ArkSyncClient
import com.kazui.delphi.ui.screens.SmartListViewModel
import dagger.hilt.android.lifecycle.HiltViewModel
import javax.inject.Inject

@HiltViewModel
class TodayViewModel @Inject constructor(
    todoDao: TodoDao,
    arkSyncClient: ArkSyncClient,
) : SmartListViewModel(todoDao, arkSyncClient, SmartList.TODAY, defaultIsToday = true)
