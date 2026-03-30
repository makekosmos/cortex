package com.kazui.delphi.ui.screens.upcoming

import androidx.compose.runtime.Composable
import androidx.hilt.navigation.compose.hiltViewModel
import com.kazui.delphi.data.model.SmartList
import com.kazui.delphi.ui.components.SmartListScaffold

@Composable
fun UpcomingScreen(
    viewModel: UpcomingViewModel = hiltViewModel(),
) {
    SmartListScaffold(
        title = "Планы",
        list = SmartList.UPCOMING,
        viewModel = viewModel,
    )
}
