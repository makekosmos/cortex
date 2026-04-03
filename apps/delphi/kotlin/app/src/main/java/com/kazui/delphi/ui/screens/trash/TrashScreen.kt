package com.kazui.delphi.ui.screens.trash

import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.kazui.delphi.data.model.SmartList
import com.kazui.delphi.ui.components.SmartListScaffold

@Composable
fun TrashScreen(
    onBack: () -> Unit,
    viewModel: TrashViewModel = hiltViewModel(),
) {
    val todos by viewModel.todos.collectAsStateWithLifecycle()
    var showConfirmDialog by remember { mutableStateOf(false) }

    SmartListScaffold(
        title = "Корзина",
        list = SmartList.TRASH,
        viewModel = viewModel,
        onBack = onBack,
        showSort = false,
        trashAction = if (todos.isNotEmpty()) {
            { showConfirmDialog = true }
        } else null,
    )

    if (showConfirmDialog) {
        AlertDialog(
            onDismissRequest = { showConfirmDialog = false },
            title = { Text("Очистить корзину") },
            text = { Text("Все задачи в корзине будут удалены безвозвратно.") },
            confirmButton = {
                TextButton(onClick = {
                    viewModel.emptyTrash()
                    showConfirmDialog = false
                }) {
                    Text("Удалить")
                }
            },
            dismissButton = {
                TextButton(onClick = { showConfirmDialog = false }) {
                    Text("Отмена")
                }
            },
        )
    }
}
