package com.kazui.delphi.ui.viewmodel

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.kazui.delphi.data.model.Priority
import com.kazui.delphi.data.model.Project
import com.kazui.delphi.data.model.SmartList
import com.kazui.delphi.data.model.TodoItem
import com.kazui.delphi.data.sync.PeerManager
import com.kazui.delphi.di.DatabaseProvider
import com.kazui.delphi.domain.filter.TodoFilterService
import com.kazui.delphi.domain.model.SmartListCounts
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import java.time.Instant
import java.util.UUID
import javax.inject.Inject

@HiltViewModel
class TodoViewModel @Inject constructor(
    private val databaseProvider: DatabaseProvider,
    private val peerManager: PeerManager,
) : ViewModel() {

    private val repo = databaseProvider.arkDataRepository

    /** True if the ark-data ContentProvider package is installed. */
    val isArkDataAvailable: Boolean get() = databaseProvider.isArkDataAvailable

    private val _selectedList = MutableStateFlow(SmartList.TODAY)
    val selectedList: StateFlow<SmartList> = _selectedList.asStateFlow()

    val allTodos: StateFlow<List<TodoItem>> = repo.getTodosFlow()
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5000), emptyList())

    val allProjects: StateFlow<List<Project>> = repo.getProjectsFlow()
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5000), emptyList())

    val filteredTodos: StateFlow<List<TodoItem>> = combine(allTodos, _selectedList) { todos, list ->
        TodoFilterService.filter(list, todos)
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5000), emptyList())

    val smartListCounts: StateFlow<SmartListCounts> = allTodos.combine(_selectedList) { todos, _ ->
        SmartListCounts(TodoFilterService.countAll(todos))
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5000), SmartListCounts())

    fun selectList(list: SmartList) {
        _selectedList.value = list
    }

    fun createTodo(title: String) {
        viewModelScope.launch {
            val todo = TodoItem(
                id = UUID.randomUUID().toString(),
                title = title,
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

    fun toggleToday(todo: TodoItem) {
        viewModelScope.launch {
            val updated = todo.copy(isToday = !todo.isToday)
            repo.upsert(updated)
            peerManager.broadcastTodoChange(updated)
        }
    }

    fun setPriority(todo: TodoItem, priority: Priority) {
        viewModelScope.launch {
            val updated = todo.copy(priority = priority)
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

    fun deleteTodo(todo: TodoItem) {
        viewModelScope.launch {
            repo.deleteById(todo.id)
            peerManager.broadcastTodoDelete(todo.id)
        }
    }
}
