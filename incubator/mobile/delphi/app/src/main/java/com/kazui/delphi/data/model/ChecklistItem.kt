package com.kazui.delphi.data.model

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.ForeignKey
import androidx.room.Index
import androidx.room.PrimaryKey

@Entity(
    tableName = "checklist_items",
    foreignKeys = [
        ForeignKey(
            entity = TodoItem::class,
            parentColumns = ["id"],
            childColumns = ["todoItemId"],
            onDelete = ForeignKey.CASCADE,
        ),
    ],
    indices = [
        Index("todoItemId"),
    ],
)
data class ChecklistItem(
    @PrimaryKey
    val id: String,
    val title: String,
    val isCompleted: Boolean = false,
    val sortOrder: Int = 0,
    @ColumnInfo(name = "todoItemId")
    val todoItemId: String,
)
