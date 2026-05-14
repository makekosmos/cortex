package com.kosmos.ark.data.model

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.ForeignKey
import androidx.room.Index
import androidx.room.PrimaryKey

@Entity(
    tableName = "todos",
    foreignKeys = [
        ForeignKey(
            entity = Heading::class,
            parentColumns = ["id"],
            childColumns = ["headingId"],
            onDelete = ForeignKey.SET_NULL,
        ),
        ForeignKey(
            entity = Project::class,
            parentColumns = ["id"],
            childColumns = ["projectId"],
            onDelete = ForeignKey.SET_NULL,
        ),
        ForeignKey(
            entity = Area::class,
            parentColumns = ["id"],
            childColumns = ["areaId"],
            onDelete = ForeignKey.SET_NULL,
        ),
    ],
    indices = [
        Index("headingId"),
        Index("projectId"),
        Index("areaId"),
    ],
)
data class TodoItem(
    @PrimaryKey
    val id: String,
    val title: String,
    val notes: String? = null,
    val priority: Priority = Priority.NONE,
    val scheduledDate: String? = null,
    val deadline: String? = null,
    val reminderDate: String? = null,
    val isToday: Boolean = false,
    val isEvening: Boolean = false,
    val isSomeday: Boolean = false,
    val isCompleted: Boolean = false,
    val completedAt: String? = null,
    val isCancelled: Boolean = false,
    val cancelledAt: String? = null,
    val isTrashed: Boolean = false,
    val sortOrder: Int = 0,
    @ColumnInfo(name = "headingId")
    val headingId: String? = null,
    @ColumnInfo(name = "projectId")
    val projectId: String? = null,
    @ColumnInfo(name = "areaId")
    val areaId: String? = null,
    val recurrenceData: RecurrenceData? = null,
    val createdAt: String,
)
