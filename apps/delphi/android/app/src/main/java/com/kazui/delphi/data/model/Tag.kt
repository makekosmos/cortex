package com.kazui.delphi.data.model

import androidx.room.Entity
import androidx.room.PrimaryKey

@Entity(tableName = "tags")
data class Tag(
    @PrimaryKey
    val id: String,
    val title: String,
    val color: String? = null,
    val createdAt: String,
)
