package com.kazui.delphi.ui.screens

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.kazui.delphi.data.model.SmartList
import com.kazui.delphi.data.model.TodoItem
import com.kazui.delphi.data.sync.PeerManager
import com.kazui.delphi.di.DatabaseProvider
import com.kazui.delphi.domain.filter.TodoFilterService
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.flowOn
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import java.time.Instant
import java.util.UUID

@OptIn(ExperimentalCoroutinesApi::class)
abstract class SmartListViewModel(
    protected val databaseProvider: DatabaseProvider,
    protected val peerManager: PeerManager,
    private val smartList: SmartList,
    private val defaultIsToday: Boolean = false,
    private val defaultIsSomeday: Boolean = false,
) : ViewModel() {

    private val repo = databaseProvider.arkDataRepository

    val todos: StateFlow<List<TodoItem>> = when (smartList) {
        SmartList.LOGBOOK -> repo.getLogbookFlow()
            .distinctUntilChanged()
            .flowOn(Dispatchers.Default)
        SmartList.TRASH -> repo.getTrashFlow()
            .distinctUntilChanged()
            .flowOn(Dispatchers.Default)
        else -> repo.getTodosFlow()
            .map { TodoFilterService.filter(smartList, it) }
            .distinctUntilChanged()
            .flowOn(Dispatchers.Default)
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5000), emptyList())

    val isReadOnly: Boolean get() = smartList == SmartList.LOGBOOK || smartList == SmartList.TRASH

    fun addTodo(title: String) {
        if (title.isBlank() || isReadOnly) return
        viewModelScope.launch {
            val todo = TodoItem(
                id = UUID.randomUUID().toString(),
                title = title.trim(),
                isToday = defaultIsToday,
                isSomeday = defaultIsSomeday,
                createdAt = Instant.now().toString(),
            )
            repo.upsert(todo)
            peerManager.broadcastTodoChange(todo)
        }
    }

    fun toggleComplete(todo: TodoItem) {
        viewModelScope.launch {
            val updated = if (todo.isCompleted) {
                todo.copy(isCompleted = false, completedAt = null)
            } else {
                todo.copy(isCompleted = true, completedAt = Instant.now().toString())
            }
            repo.upsert(updated)
            peerManager.broadcastTodoChange(updated)
        }
    }

    fun trashTodo(todo: TodoItem) {
        viewModelScope.launch {
            val updated = todo.copy(isTrashed = true)
            repo.upsert(updated)
            peerManager.broadcastTodoChange(updated)
        }
    }
}
