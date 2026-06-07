package com.kazui.delphi.ui.screens.today

import androidx.compose.runtime.Composable
import androidx.hilt.navigation.compose.hiltViewModel
import com.kazui.delphi.data.model.SmartList
import com.kazui.delphi.ui.components.SmartListScaffold

@Composable
fun TodayScreen(
    viewModel: TodayViewModel = hiltViewModel(),
) {
    SmartListScaffold(
        title = "Сегодня",
        list = SmartList.TODAY,
        viewModel = viewModel,
    )
}
