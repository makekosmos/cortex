package com.kazui.delphi.ui.screens.project

import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.kazui.delphi.data.model.Project
import com.kazui.delphi.data.model.TodoItem
import com.kazui.delphi.data.sync.ArkEventMapper
import com.kazui.delphi.data.sync.ArkSyncClient
import com.kazui.delphi.data.sync.PeerManager
import com.kazui.delphi.di.DatabaseProvider
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.flatMapLatest
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import java.time.Instant
import java.util.UUID
import javax.inject.Inject

@OptIn(ExperimentalCoroutinesApi::class)
@HiltViewModel
class ProjectViewModel @Inject constructor(
    savedStateHandle: SavedStateHandle,
    private val databaseProvider: DatabaseProvider,
    private val arkSyncClient: ArkSyncClient,
    private val peerManager: PeerManager,
) : ViewModel() {

    private val projectId: String = checkNotNull(savedStateHandle["projectId"])

    val project: StateFlow<Project?> = databaseProvider.dbGeneration
        .flatMapLatest {
            if (!databaseProvider.isOpen) return@flatMapLatest flowOf(null)
            databaseProvider.projectDao().getAllProjects()
                .map { it.find { p -> p.id == projectId } }
        }
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5000), null)

    val todos: StateFlow<List<TodoItem>> = databaseProvider.dbGeneration
        .flatMapLatest {
            if (!databaseProvider.isOpen) return@flatMapLatest flowOf(emptyList())
            databaseProvider.todoDao().getByProject(projectId)
        }
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5000), emptyList())

    fun addTodo(title: String) {
        if (title.isBlank()) return
        viewModelScope.launch {
            val todo = TodoItem(
                id = UUID.randomUUID().toString(),
                title = title.trim(),
                projectId = projectId,
                createdAt = Instant.now().toString(),
            )
            databaseProvider.todoDao().upsert(todo)
            arkSyncClient.sendChange(ArkEventMapper.todoToArkChange(todo, "create", ""))
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
            databaseProvider.todoDao().upsert(updated)
            arkSyncClient.sendChange(ArkEventMapper.todoToArkChange(updated, "update", ""))
            peerManager.broadcastTodoChange(updated)
        }
    }
}
