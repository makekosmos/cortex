package com.kazui.delphi.ui.viewmodel

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.kazui.delphi.data.model.Priority
import com.kazui.delphi.data.model.Project
import com.kazui.delphi.data.model.SmartList
import com.kazui.delphi.data.model.TodoItem
import com.kazui.delphi.data.sync.ArkEventMapper
import com.kazui.delphi.data.sync.ArkSyncClient
import com.kazui.delphi.data.sync.SyncStatus
import com.kazui.delphi.di.DatabaseProvider
import com.kazui.delphi.domain.filter.TodoFilterService
import com.kazui.delphi.domain.model.SmartListCounts
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import java.time.Instant
import java.util.UUID
import javax.inject.Inject

@OptIn(ExperimentalCoroutinesApi::class)
@HiltViewModel
class TodoViewModel @Inject constructor(
    private val databaseProvider: DatabaseProvider,
    private val syncClient: ArkSyncClient,
) : ViewModel() {

    val syncStatus: StateFlow<SyncStatus> = syncClient.status

    private val _selectedList = MutableStateFlow(SmartList.TODAY)
    val selectedList: StateFlow<SmartList> = _selectedList.asStateFlow()

    val allTodos: StateFlow<List<TodoItem>> = databaseProvider.dbGeneration
        .flatMapLatest {
            if (!databaseProvider.isOpen) return@flatMapLatest flowOf(emptyList())
            databaseProvider.todoDao().getAll()
        }
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5000), emptyList())

    val allProjects: StateFlow<List<Project>> = databaseProvider.dbGeneration
        .flatMapLatest {
            if (!databaseProvider.isOpen) return@flatMapLatest flowOf(emptyList())
            databaseProvider.projectDao().getAllProjects()
        }
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5000), emptyList())

    val filteredTodos: StateFlow<List<TodoItem>> = combine(allTodos, _selectedList) { todos, list ->
        TodoFilterService.filter(list, todos)
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5000), emptyList())

    val smartListCounts: StateFlow<SmartListCounts> = allTodos.combine(_selectedList) { todos, _ ->
        SmartListCounts(TodoFilterService.countAll(todos))
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5000), SmartListCounts())

    init {
        viewModelScope.launch {
            syncClient.onChange { change ->
                viewModelScope.launch {
                    if (!databaseProvider.isOpen) return@launch
                    when {
                        ArkEventMapper.isTaskChange(change) -> {
                            if (change.change_type == "delete") {
                                databaseProvider.todoDao().deleteById(change.data.source_id)
                            } else {
                                ArkEventMapper.arkChangeToTodoItem(change)?.let {
                                    databaseProvider.todoDao().upsert(it)
                                }
                            }
                        }
                        ArkEventMapper.isProjectChange(change) -> {
                            if (change.change_type == "delete") {
                                databaseProvider.projectDao().deleteProjectById(change.data.source_id)
                            } else {
                                ArkEventMapper.arkChangeToProject(change)?.let {
                                    databaseProvider.projectDao().upsertProject(it)
                                }
                            }
                        }
                    }
                }
            }
        }
    }

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
            databaseProvider.todoDao().upsert(todo)
            syncClient.sendChange(
                ArkEventMapper.todoToArkChange(todo, "create", "")
            )
        }
    }

    fun toggleComplete(todo: TodoItem) {
        viewModelScope.launch {
            val updated = if (todo.isCompleted) {
                todo.copy(isCompleted = false, completedAt = null)
            } else {
                todo.copy(isCompleted = true, completedAt = Instant.now().toString())
            }
            databaseProvider.todoDao().upsert(updated)
            syncClient.sendChange(
                ArkEventMapper.todoToArkChange(updated, "update", "")
            )
        }
    }

    fun toggleToday(todo: TodoItem) {
        viewModelScope.launch {
            val updated = todo.copy(isToday = !todo.isToday)
            databaseProvider.todoDao().upsert(updated)
            syncClient.sendChange(
                ArkEventMapper.todoToArkChange(updated, "update", "")
            )
        }
    }

    fun setPriority(todo: TodoItem, priority: Priority) {
        viewModelScope.launch {
            val updated = todo.copy(priority = priority)
            databaseProvider.todoDao().upsert(updated)
            syncClient.sendChange(
                ArkEventMapper.todoToArkChange(updated, "update", "")
            )
        }
    }

    fun trashTodo(todo: TodoItem) {
        viewModelScope.launch {
            val updated = todo.copy(isTrashed = true)
            databaseProvider.todoDao().upsert(updated)
            syncClient.sendChange(
                ArkEventMapper.todoToArkChange(updated, "update", "")
            )
        }
    }

    fun deleteTodo(todo: TodoItem) {
        viewModelScope.launch {
            databaseProvider.todoDao().deleteById(todo.id)
            syncClient.sendChange(
                ArkEventMapper.todoToArkChange(todo, "delete", "")
            )
        }
    }
}
