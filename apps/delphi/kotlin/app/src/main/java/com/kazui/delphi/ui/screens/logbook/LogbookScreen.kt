package com.kazui.delphi.ui.screens.logbook

import androidx.compose.runtime.Composable
import androidx.hilt.navigation.compose.hiltViewModel
import com.kazui.delphi.data.model.SmartList
import com.kazui.delphi.ui.components.SmartListScaffold

@Composable
fun LogbookScreen(
    onBack: () -> Unit,
    viewModel: LogbookViewModel = hiltViewModel(),
) {
    SmartListScaffold(
        title = "Журнал",
        list = SmartList.LOGBOOK,
        viewModel = viewModel,
        onBack = onBack,
    )
}
