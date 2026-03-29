package com.kazui.delphi.data.db

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.Query
import com.kazui.delphi.data.model.PendingChange

@Dao
interface PendingChangeDao {
    @Query("SELECT * FROM pending_changes ORDER BY id ASC")
    suspend fun getAll(): List<PendingChange>

    @Insert
    suspend fun insert(change: PendingChange)

    @Query("DELETE FROM pending_changes WHERE id = :id")
    suspend fun deleteById(id: Long)

    @Query("DELETE FROM pending_changes")
    suspend fun deleteAll()
}
