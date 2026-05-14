package com.kosmos.ark.data.db

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import androidx.room.Update
import com.kosmos.ark.data.model.Heading

@Dao
interface HeadingDao {
    @Query("SELECT * FROM headings ORDER BY sortOrder ASC")
    fun getAll(): List<Heading>

    @Query("SELECT * FROM headings WHERE id = :id LIMIT 1")
    fun getById(id: String): Heading?

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    fun insert(heading: Heading): Long

    @Update
    fun update(heading: Heading): Int

    @Query("DELETE FROM headings WHERE id = :id")
    fun deleteById(id: String): Int
}
