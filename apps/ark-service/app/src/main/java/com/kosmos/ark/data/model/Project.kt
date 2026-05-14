package com.kosmos.ark.data.model

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.ForeignKey
import androidx.room.Index
import androidx.room.PrimaryKey

@Entity(
    tableName = "projects",
    foreignKeys = [
        ForeignKey(
            entity = Area::class,
            parentColumns = ["id"],
            childColumns = ["areaId"],
            onDelete = ForeignKey.SET_NULL,
        ),
    ],
    indices = [
        Index("areaId"),
    ],
)
data class Project(
    @PrimaryKey
    val id: String,
    val title: String,
    val notes: String? = null,
    val status: ProjectStatus = ProjectStatus.ACTIVE,
    val scheduledDate: String? = null,
    val deadline: String? = null,
    val sortOrder: Int = 0,
    val colorTag: String? = null,
    @ColumnInfo(name = "areaId")
    val areaId: String? = null,
    val createdAt: String,
)
