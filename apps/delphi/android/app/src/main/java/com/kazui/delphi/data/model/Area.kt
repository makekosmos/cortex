package com.kazui.delphi.data.model

import androidx.room.Entity
import androidx.room.PrimaryKey

@Entity(tableName = "areas")
data class Area(
    @PrimaryKey
    val id: String,
    val title: String,
    val sortOrder: Int = 0,
    val createdAt: String,
)
