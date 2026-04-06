package com.kepler.ark.data.db

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import androidx.room.Update
import com.kepler.ark.data.model.TodoItem

@Dao
interface TodoDao {
    @Query("SELECT * FROM todos ORDER BY createdAt DESC")
    fun getAll(): List<TodoItem>

    @Query("SELECT * FROM todos WHERE id = :id LIMIT 1")
    fun getById(id: String): TodoItem?

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    fun insert(todo: TodoItem): Long

    @Update
    fun update(todo: TodoItem): Int

    @Query("DELETE FROM todos WHERE id = :id")
    fun deleteById(id: String): Int
}
