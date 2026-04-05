package com.kepler.ark.data.db

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import androidx.room.Update
import com.kepler.ark.data.model.ChecklistItem

@Dao
interface ChecklistItemDao {
    @Query("SELECT * FROM checklist_items ORDER BY sortOrder ASC")
    fun getAll(): List<ChecklistItem>

    @Query("SELECT * FROM checklist_items WHERE id = :id LIMIT 1")
    fun getById(id: String): ChecklistItem?

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    fun insert(item: ChecklistItem): Long

    @Update
    fun update(item: ChecklistItem): Int

    @Query("DELETE FROM checklist_items WHERE id = :id")
    fun deleteById(id: String): Int
}
