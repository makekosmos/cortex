package com.kepler.ark.data.db

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import androidx.room.Update
import com.kepler.ark.data.model.Area

@Dao
interface AreaDao {
    @Query("SELECT * FROM areas ORDER BY sortOrder ASC")
    fun getAll(): List<Area>

    @Query("SELECT * FROM areas WHERE id = :id LIMIT 1")
    fun getById(id: String): Area?

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    fun insert(area: Area): Long

    @Update
    fun update(area: Area): Int

    @Query("DELETE FROM areas WHERE id = :id")
    fun deleteById(id: String): Int
}
