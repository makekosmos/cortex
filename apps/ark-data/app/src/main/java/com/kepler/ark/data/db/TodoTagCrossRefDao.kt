package com.kepler.ark.data.db

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import com.kepler.ark.data.model.TodoTagCrossRef

@Dao
interface TodoTagCrossRefDao {
    @Query("SELECT * FROM todo_tag_cross_ref")
    fun getAll(): List<TodoTagCrossRef>

    @Query("SELECT * FROM todo_tag_cross_ref WHERE todoId = :todoId AND tagId = :tagId LIMIT 1")
    fun getById(todoId: String, tagId: String): TodoTagCrossRef?

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    fun insert(ref: TodoTagCrossRef): Long

    @Query("DELETE FROM todo_tag_cross_ref WHERE todoId = :todoId AND tagId = :tagId")
    fun deleteById(todoId: String, tagId: String): Int
}
