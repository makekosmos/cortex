package com.kazui.delphi.data.sync

import com.kazui.delphi.data.model.Priority
import com.kazui.delphi.data.model.Project
import com.kazui.delphi.data.model.ProjectStatus
import com.kazui.delphi.data.model.TodoItem
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.boolean
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.int
import kotlinx.serialization.json.intOrNull
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.put
import java.time.Instant

@Serializable
data class ArkChange(
    val event_id: String,
    val change_type: String,
    val data: ArkEventData,
    val device_id: String? = null,
    val device_seq: Long? = null,
)

@Serializable
data class ArkEventData(
    val event_type: String,
    val category: String = "productivity",
    val source: String = "delphi-android",
    val source_id: String = "",
    val summary: String = "",
    val occurred_at: String = "",
    val data: JsonObject = kotlinx.serialization.json.JsonObject(emptyMap()),
)

object ArkEventMapper {

    fun todoToArkChange(todo: TodoItem, changeType: String, deviceId: String): ArkChange {
        val dataMap = buildJsonObject {
            put("id", todo.id)
            put("title", todo.title)
            todo.notes?.let { put("notes", it) }
            put("priority", todo.priority.value)
            todo.scheduledDate?.let { put("scheduledDate", it) }
            todo.deadline?.let { put("deadline", it) }
            todo.reminderDate?.let { put("reminderDate", it) }
            put("isToday", todo.isToday)
            put("isEvening", todo.isEvening)
            put("isSomeday", todo.isSomeday)
            put("isCompleted", todo.isCompleted)
            todo.completedAt?.let { put("completedAt", it) }
            put("isCancelled", todo.isCancelled)
            todo.cancelledAt?.let { put("cancelledAt", it) }
            put("isTrashed", todo.isTrashed)
            put("sortOrder", todo.sortOrder)
            todo.headingId?.let { put("headingId", it) }
            todo.projectId?.let { put("projectId", it) }
            todo.areaId?.let { put("areaId", it) }
            put("createdAt", todo.createdAt)
        }
        return ArkChange(
            event_id = todo.id,
            change_type = changeType,
            data = ArkEventData(
                event_type = "task",
                source_id = todo.id,
                summary = todo.title,
                occurred_at = Instant.now().toString(),
                data = dataMap,
            ),
            device_id = deviceId,
        )
    }

    fun arkChangeToTodoItem(change: ArkChange): TodoItem? {
        return try {
            val d = change.data.data
            val id = (d["id"]?.jsonPrimitive?.contentOrNull
                ?: change.data.source_id.ifBlank { null }
                ?: change.event_id).lowercase()
            val title = d["title"]?.jsonPrimitive?.contentOrNull
                ?: change.data.summary.ifBlank { null }
                ?: return null
            TodoItem(
                id = id,
                title = title,
                notes = d["notes"]?.jsonPrimitive?.contentOrNull,
                priority = Priority.entries.firstOrNull {
                    it.value == d["priority"]?.jsonPrimitive?.intOrNull
                } ?: Priority.NONE,
                scheduledDate = d["scheduledDate"]?.jsonPrimitive?.contentOrNull,
                deadline = d["deadline"]?.jsonPrimitive?.contentOrNull,
                reminderDate = d["reminderDate"]?.jsonPrimitive?.contentOrNull,
                isToday = d["isToday"]?.jsonPrimitive?.booleanOrNull ?: false,
                isEvening = d["isEvening"]?.jsonPrimitive?.booleanOrNull ?: false,
                isSomeday = d["isSomeday"]?.jsonPrimitive?.booleanOrNull ?: false,
                isCompleted = d["isCompleted"]?.jsonPrimitive?.booleanOrNull
                    ?: d["completed"]?.jsonPrimitive?.booleanOrNull ?: false,
                completedAt = d["completedAt"]?.jsonPrimitive?.contentOrNull
                    ?: d["completed_at"]?.jsonPrimitive?.contentOrNull,
                isCancelled = d["isCancelled"]?.jsonPrimitive?.booleanOrNull ?: false,
                cancelledAt = d["cancelledAt"]?.jsonPrimitive?.contentOrNull,
                isTrashed = d["isTrashed"]?.jsonPrimitive?.booleanOrNull ?: false,
                sortOrder = d["sortOrder"]?.jsonPrimitive?.intOrNull ?: 0,
                headingId = d["headingId"]?.jsonPrimitive?.contentOrNull,
                projectId = d["projectId"]?.jsonPrimitive?.contentOrNull,
                areaId = d["areaId"]?.jsonPrimitive?.contentOrNull,
                createdAt = d["createdAt"]?.jsonPrimitive?.content ?: Instant.now().toString(),
            )
        } catch (_: Exception) {
            null
        }
    }

    fun projectToArkChange(project: Project, changeType: String, deviceId: String): ArkChange {
        val dataMap = buildJsonObject {
            put("id", project.id)
            put("title", project.title)
            project.notes?.let { put("notes", it) }
            put("status", project.status.value)
            project.scheduledDate?.let { put("scheduledDate", it) }
            project.deadline?.let { put("deadline", it) }
            put("sortOrder", project.sortOrder)
            project.colorTag?.let { put("colorTag", it) }
            project.areaId?.let { put("areaId", it) }
            put("createdAt", project.createdAt)
        }
        return ArkChange(
            event_id = project.id,
            change_type = changeType,
            data = ArkEventData(
                event_type = "project",
                source_id = project.id,
                summary = project.title,
                occurred_at = Instant.now().toString(),
                data = dataMap,
            ),
            device_id = deviceId,
        )
    }

    fun arkChangeToProject(change: ArkChange): Project? {
        return try {
            val d = change.data.data
            Project(
                id = (d["id"]?.jsonPrimitive?.content ?: change.data.source_id).lowercase(),
                title = d["title"]?.jsonPrimitive?.content ?: return null,
                notes = d["notes"]?.jsonPrimitive?.contentOrNull,
                status = ProjectStatus.entries.firstOrNull {
                    it.value == d["status"]?.jsonPrimitive?.intOrNull
                } ?: ProjectStatus.ACTIVE,
                scheduledDate = d["scheduledDate"]?.jsonPrimitive?.contentOrNull,
                deadline = d["deadline"]?.jsonPrimitive?.contentOrNull,
                sortOrder = d["sortOrder"]?.jsonPrimitive?.intOrNull ?: 0,
                colorTag = d["colorTag"]?.jsonPrimitive?.contentOrNull,
                areaId = d["areaId"]?.jsonPrimitive?.contentOrNull,
                createdAt = d["createdAt"]?.jsonPrimitive?.content ?: Instant.now().toString(),
            )
        } catch (_: Exception) {
            null
        }
    }

    fun isTaskChange(change: ArkChange): Boolean =
        change.data.event_type in listOf("task", "task_created")

    fun isProjectChange(change: ArkChange): Boolean =
        change.data.event_type == "project"
}
