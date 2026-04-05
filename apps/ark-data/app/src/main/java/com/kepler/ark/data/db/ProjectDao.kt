package com.kepler.ark.data.db

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import androidx.room.Update
import com.kepler.ark.data.model.Project

@Dao
interface ProjectDao {
    @Query("SELECT * FROM projects ORDER BY sortOrder ASC")
    fun getAll(): List<Project>

    @Query("SELECT * FROM projects WHERE id = :id LIMIT 1")
    fun getById(id: String): Project?

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    fun insert(project: Project): Long

    @Update
    fun update(project: Project): Int

    @Query("DELETE FROM projects WHERE id = :id")
    fun deleteById(id: String): Int
}
