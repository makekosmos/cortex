package com.kepler.ark.data.model

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.ForeignKey
import androidx.room.Index

@Entity(
    tableName = "todo_tag_cross_ref",
    primaryKeys = ["todoId", "tagId"],
    foreignKeys = [
        ForeignKey(
            entity = TodoItem::class,
            parentColumns = ["id"],
            childColumns = ["todoId"],
            onDelete = ForeignKey.CASCADE,
        ),
        ForeignKey(
            entity = Tag::class,
            parentColumns = ["id"],
            childColumns = ["tagId"],
            onDelete = ForeignKey.CASCADE,
        ),
    ],
    indices = [
        Index("todoId"),
        Index("tagId"),
    ],
)
data class TodoTagCrossRef(
    @ColumnInfo(name = "todoId")
    val todoId: String,
    @ColumnInfo(name = "tagId")
    val tagId: String,
)
