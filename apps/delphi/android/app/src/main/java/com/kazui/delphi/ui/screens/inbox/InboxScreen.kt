package com.kazui.delphi.ui.screens.inbox

import androidx.compose.runtime.Composable
import androidx.hilt.navigation.compose.hiltViewModel
import com.kazui.delphi.data.model.SmartList
import com.kazui.delphi.ui.components.SmartListScaffold

@Composable
fun InboxScreen(
    onOpenSettings: () -> Unit,
    viewModel: InboxViewModel = hiltViewModel(),
) {
    SmartListScaffold(
        title = "Входящие",
        list = SmartList.INBOX,
        viewModel = viewModel,
        onOpenSettings = onOpenSettings,
    )
}
