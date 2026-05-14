package com.kazui.delphi.data.sync

import android.util.Log
import com.kazui.delphi.data.model.ChecklistItem
import com.kazui.delphi.data.model.TodoItem
import com.kazui.delphi.data.model.Project
import com.kazui.delphi.data.model.Area
import com.kazui.delphi.data.model.Heading
import com.kazui.delphi.data.model.Tag
import com.kazui.delphi.data.model.TodoTagCrossRef
import com.kazui.delphi.data.repository.ArkDataRepository
import org.json.JSONArray
import org.json.JSONObject
import java.time.Instant
import java.util.UUID

private const val TAG = "SyncEntityParser"

/**
 * Shared JSON <-> model converters for sync entities.
 * Used by both SyncServer and LanSyncClient to avoid duplication.
 */
object SyncEntityParser {

    fun todoToJson(
        todo: TodoItem,
        tagIds: List<String> = emptyList(),
        checklistItems: List<ChecklistItem> = emptyList(),
    ): JSONObject {
        return JSONObject().apply {
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
            put("tagIds", JSONArray(tagIds))
            put("checklistItems", JSONArray().apply {
                for (item in checklistItems) {
                    put(JSONObject().apply {
                        put("id", item.id)
                        put("title", item.title)
                        put("isCompleted", item.isCompleted)
                        put("sortOrder", item.sortOrder)
                    })
                }
            })
        }
    }

    fun areaToJson(area: Area): JSONObject {
        return JSONObject().apply {
            put("id", area.id)
            put("title", area.title)
            put("sortOrder", area.sortOrder)
            put("createdAt", area.createdAt)
        }
    }

    fun tagToJson(tag: Tag): JSONObject {
        return JSONObject().apply {
            put("id", tag.id)
            put("title", tag.title)
            tag.color?.let { put("color", it) }
            put("createdAt", tag.createdAt)
        }
    }

    fun headingToJson(heading: Heading): JSONObject {
        return JSONObject().apply {
            put("id", heading.id)
            put("title", heading.title)
            put("sortOrder", heading.sortOrder)
            put("projectId", heading.projectId)
        }
    }

    /**
     * Apply checklist items and tag cross-refs from incoming sync JSON data.
     * Replaces existing checklist items for this todo and syncs tag refs.
     */
    suspend fun applyChecklistAndTags(data: JSONObject, todoId: String, repo: ArkDataRepository) {
        try {
            // Apply checklist items
            val checklistArr = data.optJSONArray("checklistItems")
            if (checklistArr != null && checklistArr.length() > 0) {
                // Delete existing checklist items for this todo first
                repo.deleteChecklistItemsByTodoIds(listOf(todoId))
                for (i in 0 until checklistArr.length()) {
                    val itemJson = checklistArr.optJSONObject(i) ?: continue
                    val item = ChecklistItem(
                        id = itemJson.optString("id", UUID.randomUUID().toString()).lowercase(),
                        title = itemJson.optString("title", ""),
                        isCompleted = itemJson.optBoolean("isCompleted", false),
                        sortOrder = itemJson.optInt("sortOrder", i),
                        todoItemId = todoId,
                    )
                    if (item.title.isNotEmpty()) {
                        repo.upsertChecklistItem(item)
                    }
                }
            }

            // Apply tag cross-refs
            val tagIdsArr = data.optJSONArray("tagIds")
            if (tagIdsArr != null && tagIdsArr.length() > 0) {
                // Delete existing tag refs for this todo first
                repo.deleteTagRefsByTodoIds(listOf(todoId))
                for (i in 0 until tagIdsArr.length()) {
                    val tagId = tagIdsArr.optString(i, "") .lowercase()
                    if (tagId.isNotEmpty()) {
                        repo.upsertCrossRef(TodoTagCrossRef(todoId = todoId, tagId = tagId))
                    }
                }
            }
        } catch (e: Exception) {
            Log.e(TAG, "Failed to apply checklist/tags for $todoId: ${e.message}")
        }
    }

    fun projectToJson(project: Project): JSONObject {
        return JSONObject().apply {
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
    }

    fun jsonToTodoItem(json: JSONObject, fallbackId: String): TodoItem? {
        return try {
            val id = json.optString("id", fallbackId).lowercase()
            val title = json.optString("title", "")
            if (title.isEmpty()) return null

            TodoItem(
                id = id,
                title = title,
                notes = json.optStringOrNull("notes"),
                priority = com.kazui.delphi.data.model.Priority.entries.firstOrNull {
                    it.value == json.optInt("priority", 0)
                } ?: com.kazui.delphi.data.model.Priority.NONE,
                scheduledDate = json.optStringOrNull("scheduledDate"),
                deadline = json.optStringOrNull("deadline"),
                reminderDate = json.optStringOrNull("reminderDate"),
                isToday = json.optBoolean("isToday", false),
                isEvening = json.optBoolean("isEvening", false),
                isSomeday = json.optBoolean("isSomeday", false),
                isCompleted = json.optBoolean("isCompleted", json.optBoolean("completed", false)),
                completedAt = json.optStringOrNull("completedAt") ?: json.optStringOrNull("completed_at"),
                isCancelled = json.optBoolean("isCancelled", false),
                cancelledAt = json.optStringOrNull("cancelledAt"),
                isTrashed = json.optBoolean("isTrashed", false),
                sortOrder = json.optInt("sortOrder", 0),
                headingId = json.optStringOrNull("headingId"),
                projectId = json.optStringOrNull("projectId"),
                areaId = json.optStringOrNull("areaId"),
                createdAt = json.optString("createdAt", Instant.now().toString()),
            )
        } catch (e: Exception) {
            Log.e(TAG, "Failed to parse TodoItem: ${e.message}")
            null
        }
    }

    fun jsonToProject(json: JSONObject, fallbackId: String): Project? {
        return try {
            Project(
                id = json.optString("id", fallbackId).lowercase(),
                title = json.optString("title", "").ifEmpty { return null },
                notes = json.optStringOrNull("notes"),
                status = com.kazui.delphi.data.model.ProjectStatus.entries.firstOrNull {
                    it.value == json.optInt("status", 0)
                } ?: com.kazui.delphi.data.model.ProjectStatus.ACTIVE,
                scheduledDate = json.optStringOrNull("scheduledDate"),
                deadline = json.optStringOrNull("deadline"),
                sortOrder = json.optInt("sortOrder", 0),
                colorTag = json.optStringOrNull("colorTag"),
                areaId = json.optStringOrNull("areaId"),
                createdAt = json.optString("createdAt", Instant.now().toString()),
            )
        } catch (e: Exception) {
            Log.e(TAG, "Failed to parse Project: ${e.message}")
            null
        }
    }

    fun jsonToArea(json: JSONObject, fallbackId: String): Area? {
        return try {
            Area(
                id = json.optString("id", fallbackId).lowercase(),
                title = json.optString("title", "").ifEmpty { return null },
                sortOrder = json.optInt("sortOrder", 0),
                createdAt = json.optString("createdAt", Instant.now().toString()),
            )
        } catch (_: Exception) { null }
    }

    fun jsonToTag(json: JSONObject, fallbackId: String): Tag? {
        return try {
            Tag(
                id = json.optString("id", fallbackId).lowercase(),
                title = json.optString("title", "").ifEmpty { return null },
                color = json.optStringOrNull("color"),
                createdAt = json.optString("createdAt", Instant.now().toString()),
            )
        } catch (_: Exception) { null }
    }

    fun jsonToHeading(json: JSONObject, fallbackId: String): Heading? {
        return try {
            Heading(
                id = json.optString("id", fallbackId).lowercase(),
                title = json.optString("title", "").ifEmpty { return null },
                sortOrder = json.optInt("sortOrder", 0),
                projectId = json.optString("projectId", ""),
            )
        } catch (_: Exception) { null }
    }

    /** Extension to return null for JSONObject.NULL and empty strings */
    private fun JSONObject.optStringOrNull(key: String): String? {
        if (isNull(key)) return null
        val value = optString(key, "")
        return value.ifEmpty { null }
    }
}
