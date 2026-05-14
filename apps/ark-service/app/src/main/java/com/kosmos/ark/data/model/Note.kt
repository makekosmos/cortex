package com.kosmos.ark.data.model

import androidx.room.Entity
import androidx.room.PrimaryKey

@Entity(tableName = "notes")
data class Note(
    @PrimaryKey
    val id: String,
    val title: String,
    val body: String,
    val createdAt: String,
    val updatedAt: String,
    val isTrashed: Int = 0,
)
