package com.kazui.delphi.data.model

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.ForeignKey
import androidx.room.Index
import androidx.room.PrimaryKey

@Entity(
    tableName = "headings",
    foreignKeys = [
        ForeignKey(
            entity = Project::class,
            parentColumns = ["id"],
            childColumns = ["projectId"],
            onDelete = ForeignKey.CASCADE,
        ),
    ],
    indices = [
        Index("projectId"),
    ],
)
data class Heading(
    @PrimaryKey
    val id: String,
    val title: String,
    val sortOrder: Int = 0,
    @ColumnInfo(name = "projectId")
    val projectId: String,
)
