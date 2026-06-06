package com.kosmos.ark.data.db

import androidx.room.TypeConverter
import com.kosmos.ark.data.model.Priority
import com.kosmos.ark.data.model.ProjectStatus
import com.kosmos.ark.data.model.RecurrenceData
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json

class Converters {
    private val json = Json { ignoreUnknownKeys = true }

    @TypeConverter
    fun priorityToInt(priority: Priority): Int = priority.value

    @TypeConverter
    fun intToPriority(value: Int): Priority = Priority.fromValue(value)

    @TypeConverter
    fun statusToInt(status: ProjectStatus): Int = status.value

    @TypeConverter
    fun intToStatus(value: Int): ProjectStatus = ProjectStatus.fromValue(value)

    @TypeConverter
    fun recurrenceToString(recurrence: RecurrenceData?): String? =
        recurrence?.let { json.encodeToString(it) }

    @TypeConverter
    fun stringToRecurrence(value: String?): RecurrenceData? =
        value?.let { json.decodeFromString(it) }
}
