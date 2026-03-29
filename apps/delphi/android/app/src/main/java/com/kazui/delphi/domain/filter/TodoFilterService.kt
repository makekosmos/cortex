package com.kazui.delphi.domain.filter

import com.kazui.delphi.data.model.SmartList
import com.kazui.delphi.data.model.TodoItem
import java.time.LocalDate
import java.time.format.DateTimeFormatter

object TodoFilterService {

    private val dateFormatter = DateTimeFormatter.ISO_DATE

    private fun today(): String = LocalDate.now().format(dateFormatter)

    private fun isActive(todo: TodoItem) =
        !todo.isCompleted && !todo.isCancelled && !todo.isTrashed

    private fun predicate(list: SmartList, todo: TodoItem): Boolean {
        val todayStr = today()
        return when (list) {
            SmartList.INBOX -> todo.projectId == null && !todo.isSomeday && isActive(todo)
            SmartList.TODAY -> isActive(todo) && (todo.isToday || todo.scheduledDate == todayStr)
            SmartList.UPCOMING -> {
                val sd = todo.scheduledDate ?: return false
                isActive(todo) && !todo.isSomeday && sd > todayStr
            }
            SmartList.ANYTIME -> isActive(todo) && !todo.isSomeday
            SmartList.SOMEDAY -> todo.isSomeday && !todo.isCompleted && !todo.isCancelled && !todo.isTrashed
            SmartList.LOGBOOK -> todo.isCompleted || todo.isCancelled
            SmartList.TRASH -> todo.isTrashed
        }
    }

    private fun sort(list: SmartList, todos: List<TodoItem>): List<TodoItem> = when (list) {
        SmartList.INBOX -> todos.sortedByDescending { it.createdAt }
        SmartList.TODAY -> todos.sortedBy { it.sortOrder }
        SmartList.UPCOMING -> todos.sortedWith(compareBy(nullsLast()) { it.scheduledDate })
        SmartList.ANYTIME -> todos.sortedByDescending { it.createdAt }
        SmartList.SOMEDAY -> todos.sortedByDescending { it.createdAt }
        SmartList.LOGBOOK -> todos.sortedByDescending { it.completedAt ?: it.cancelledAt }
        SmartList.TRASH -> todos.sortedByDescending { it.createdAt }
    }

    fun filter(list: SmartList, todos: List<TodoItem>): List<TodoItem> =
        sort(list, todos.filter { predicate(list, it) })

    fun count(list: SmartList, todos: List<TodoItem>): Int =
        todos.count { predicate(list, it) }

    fun countAll(todos: List<TodoItem>): Map<SmartList, Int> =
        SmartList.entries.associateWith { count(it, todos) }
}
