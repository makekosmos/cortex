package com.kepler.ark.data.db

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import androidx.room.Update
import com.kepler.ark.data.model.Tag

@Dao
interface TagDao {
    @Query("SELECT * FROM tags ORDER BY title ASC")
    fun getAll(): List<Tag>

    @Query("SELECT * FROM tags WHERE id = :id LIMIT 1")
    fun getById(id: String): Tag?

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    fun insert(tag: Tag): Long

    @Update
    fun update(tag: Tag): Int

    @Query("DELETE FROM tags WHERE id = :id")
    fun deleteById(id: String): Int
}
