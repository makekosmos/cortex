package com.kazui.delphi.ui.screens.more

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.kazui.delphi.data.model.Project
import com.kazui.delphi.data.model.SmartList
import com.kazui.delphi.di.DatabaseProvider
import com.kazui.delphi.domain.filter.TodoFilterService
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import javax.inject.Inject

data class ProjectWithCount(val project: Project, val taskCount: Int)

data class MoreState(
    val projects: List<ProjectWithCount> = emptyList(),
    val logbookCount: Int = 0,
    val trashCount: Int = 0,
)

@HiltViewModel
class MoreViewModel @Inject constructor(
    databaseProvider: DatabaseProvider,
) : ViewModel() {

    private val repo = databaseProvider.arkDataRepository

    val state: StateFlow<MoreState> = combine(
        repo.getProjectsFlow(),
        repo.getTodosFlow(),
    ) { projects, todos ->
        val projectsWithCounts = projects.map { project ->
            val count = todos.count { it.projectId == project.id && !it.isCompleted && !it.isTrashed }
            ProjectWithCount(project, count)
        }
        MoreState(
            projects = projectsWithCounts,
            logbookCount = TodoFilterService.count(SmartList.LOGBOOK, todos),
            trashCount = TodoFilterService.count(SmartList.TRASH, todos),
        )
    }.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5000), MoreState())
}
