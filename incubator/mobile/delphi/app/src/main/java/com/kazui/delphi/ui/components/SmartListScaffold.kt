package com.kazui.delphi.ui.components

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListPrefetchStrategy
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.Sort
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.DeleteForever
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.kazui.delphi.data.model.SmartList
import com.kazui.delphi.ui.screens.SmartListViewModel

@OptIn(ExperimentalMaterial3Api::class, ExperimentalFoundationApi::class)
@Composable
fun SmartListScaffold(
    title: String,
    list: SmartList,
    viewModel: SmartListViewModel,
    onBack: (() -> Unit)? = null,
    showSort: Boolean = true,
    trashAction: (() -> Unit)? = null,
) {
    val todos by viewModel.todos.collectAsStateWithLifecycle()
    var showInput by remember { mutableStateOf(false) }
    var inputText by remember { mutableStateOf("") }

    Scaffold(
        contentWindowInsets = WindowInsets(0, 0, 0, 0),
        topBar = {
            TopAppBar(
                title = { Text(title) },
                navigationIcon = {
                    if (onBack != null) {
                        IconButton(onClick = onBack) {
                            Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Назад")
                        }
                    }
                },
                actions = {
                    if (trashAction != null) {
                        IconButton(onClick = trashAction) {
                            Icon(Icons.Default.DeleteForever, contentDescription = "Очистить корзину")
                        }
                    }
                    if (showSort) {
                        IconButton(onClick = { /* TODO: sort */ }) {
                            Icon(Icons.AutoMirrored.Filled.Sort, contentDescription = "Сортировка")
                        }
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = MaterialTheme.colorScheme.background,
                ),
            )
        },
        floatingActionButton = {
            if (!showInput && !viewModel.isReadOnly) {
                FloatingActionButton(
                    onClick = { showInput = true },
                    containerColor = MaterialTheme.colorScheme.primary,
                ) {
                    Icon(Icons.Default.Add, contentDescription = "Добавить задачу")
                }
            }
        },
    ) { paddingValues ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(paddingValues)
                .imePadding(),
        ) {
            if (todos.isEmpty() && !showInput) {
                Box(
                    modifier = Modifier
                        .weight(1f)
                        .fillMaxSize(),
                    contentAlignment = androidx.compose.ui.Alignment.Center,
                ) {
                    Text(
                        text = if (list == SmartList.LOGBOOK) "Журнал пуст" else "Нет задач",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            } else {
                val dividerColor = MaterialTheme.colorScheme.outline.copy(alpha = 0.3f)
                val listState = rememberLazyListState(
                    prefetchStrategy = LazyListPrefetchStrategy(8),
                )
                LazyColumn(
                    state = listState,
                    modifier = Modifier.weight(1f),
                    contentPadding = PaddingValues(vertical = 4.dp),
                ) {
                    items(todos, key = { it.id }, contentType = { "todo" }) { todo ->
                        TodoRow(
                            todo = todo,
                            onToggleCompleted = { viewModel.toggleComplete(todo) },
                            onClick = { },
                            onDelete = if (!viewModel.isReadOnly) {
                                { viewModel.trashTodo(todo) }
                            } else null,
                        )
                        HorizontalDivider(
                            color = dividerColor,
                            thickness = 0.5.dp,
                            modifier = Modifier.padding(start = 56.dp),
                        )
                    }
                }
            }

            AnimatedVisibility(
                visible = showInput,
                enter = slideInVertically(initialOffsetY = { it }) + fadeIn(),
                exit = slideOutVertically(targetOffsetY = { it }) + fadeOut(),
            ) {
                QuickAddBar(
                    value = inputText,
                    onValueChange = { inputText = it },
                    onSubmit = {
                        viewModel.addTodo(inputText)
                        inputText = ""
                        showInput = false
                    },
                    onDismiss = {
                        inputText = ""
                        showInput = false
                    },
                )
            }
        }
    }
}
