package com.kazui.delphi.ui.screens.trash

import androidx.compose.runtime.Composable
import androidx.hilt.navigation.compose.hiltViewModel
import com.kazui.delphi.data.model.SmartList
import com.kazui.delphi.ui.components.SmartListScaffold

@Composable
fun TrashScreen(
    onBack: () -> Unit,
    viewModel: TrashViewModel = hiltViewModel(),
) {
    SmartListScaffold(
        title = "Корзина",
        list = SmartList.TRASH,
        viewModel = viewModel,
        onBack = onBack,
        showSort = false,
    )
}
