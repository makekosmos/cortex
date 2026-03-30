package com.kazui.delphi.ui.screens.more

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.kazui.delphi.data.db.ProjectDao
import com.kazui.delphi.data.db.TodoDao
import com.kazui.delphi.data.model.Project
import com.kazui.delphi.data.model.SmartList
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
    todoDao: TodoDao,
    projectDao: ProjectDao,
) : ViewModel() {

    val state: StateFlow<MoreState> =
        combine(projectDao.getAllProjects(), todoDao.getAll()) { projects, todos ->
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
