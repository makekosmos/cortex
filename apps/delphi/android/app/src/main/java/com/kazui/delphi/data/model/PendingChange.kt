package com.kazui.delphi.data.model

import androidx.room.Entity
import androidx.room.PrimaryKey

@Entity(tableName = "pending_changes")
data class PendingChange(
    @PrimaryKey(autoGenerate = true)
    val id: Long = 0,
    val payload: String,
    val createdAt: String,
)
