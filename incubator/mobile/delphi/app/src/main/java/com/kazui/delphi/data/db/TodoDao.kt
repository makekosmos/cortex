package com.kazui.delphi.data.db

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import com.kazui.delphi.data.model.ChecklistItem
import com.kazui.delphi.data.model.TodoItem
import com.kazui.delphi.data.model.TodoTagCrossRef
import kotlinx.coroutines.flow.Flow

@Dao
interface TodoDao {
    @Query("SELECT * FROM todos ORDER BY createdAt DESC")
    fun getAll(): Flow<List<TodoItem>>

    @Query("SELECT * FROM todos WHERE id = :id")
    suspend fun getById(id: String): TodoItem?

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsert(todo: TodoItem)

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsertAll(todos: List<TodoItem>)

    @Query("DELETE FROM todos WHERE id = :id")
    suspend fun deleteById(id: String)

    @Query("SELECT * FROM checklist_items WHERE todoItemId = :todoItemId ORDER BY sortOrder ASC")
    suspend fun getChecklistItems(todoItemId: String): List<ChecklistItem>

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsertChecklistItem(item: ChecklistItem)

    @Query("DELETE FROM checklist_items WHERE id = :id")
    suspend fun deleteChecklistItem(id: String)

    @Query("DELETE FROM checklist_items WHERE todoItemId = :todoItemId")
    suspend fun deleteChecklistItemsByTodoId(todoItemId: String)

    @Query("SELECT tagId FROM todo_tag_cross_ref WHERE todoId = :todoId")
    suspend fun getTagIdsForTodo(todoId: String): List<String>

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun insertTodoTagCrossRef(ref: TodoTagCrossRef)

    @Query("DELETE FROM todo_tag_cross_ref WHERE todoId = :todoId AND tagId = :tagId")
    suspend fun deleteTodoTagCrossRef(todoId: String, tagId: String)

    @Query("DELETE FROM todo_tag_cross_ref WHERE todoId = :todoId")
    suspend fun deleteAllTodoTagCrossRefs(todoId: String)

    @Query("SELECT * FROM todos WHERE projectId = :projectId AND isTrashed = 0 ORDER BY sortOrder ASC")
    fun getByProject(projectId: String): Flow<List<TodoItem>>

    @Query("SELECT * FROM todos WHERE (isCompleted = 1 OR isCancelled = 1) AND isTrashed = 0 ORDER BY COALESCE(completedAt, cancelledAt) DESC")
    fun getLogbook(): Flow<List<TodoItem>>

    @Query("SELECT * FROM todos WHERE isTrashed = 1 ORDER BY createdAt DESC")
    fun getTrash(): Flow<List<TodoItem>>

    @Query("SELECT * FROM todos WHERE isTrashed = 0 ORDER BY createdAt ASC")
    suspend fun getAllForSync(): List<TodoItem>

    /** Return IDs of all trashed todos (used to cascade-delete related rows). */
    @Query("SELECT id FROM todos WHERE isTrashed = 1")
    suspend fun getTrashedIds(): List<String>

    /** Permanently delete all trashed todos. */
    @Query("DELETE FROM todos WHERE isTrashed = 1")
    suspend fun deleteTrashed()

    /** Delete checklist items belonging to any of the given todo IDs. */
    @Query("DELETE FROM checklist_items WHERE todoItemId IN (:todoIds)")
    suspend fun deleteChecklistItemsByTodoIds(todoIds: List<String>)

    /** Delete tag cross-refs belonging to any of the given todo IDs. */
    @Query("DELETE FROM todo_tag_cross_ref WHERE todoId IN (:todoIds)")
    suspend fun deleteTagRefsByTodoIds(todoIds: List<String>)

    @Query("DELETE FROM todos")
    suspend fun deleteAll()

    @Query("DELETE FROM checklist_items")
    suspend fun deleteAllChecklistItems()

    @Query("DELETE FROM todo_tag_cross_ref")
    suspend fun deleteAllTagRefs()
}
