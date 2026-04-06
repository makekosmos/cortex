package com.kazui.delphi.data.repository

import android.content.ContentResolver
import android.content.ContentValues
import android.content.Context
import android.content.pm.PackageManager
import android.database.ContentObserver
import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.util.Log
import com.kazui.delphi.data.model.Area
import com.kazui.delphi.data.model.ChecklistItem
import com.kazui.delphi.data.model.Heading
import com.kazui.delphi.data.model.Priority
import com.kazui.delphi.data.model.Project
import com.kazui.delphi.data.model.ProjectStatus
import com.kazui.delphi.data.model.RecurrenceData
import com.kazui.delphi.data.model.Tag
import com.kazui.delphi.data.model.TodoItem
import com.kazui.delphi.data.model.TodoTagCrossRef
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.channels.awaitClose
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.callbackFlow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import javax.inject.Inject
import javax.inject.Singleton

private const val TAG = "ArkDataRepository"
private const val ARK_DATA_PACKAGE = "com.kepler.ark.data"
private const val AUTHORITY = "com.kepler.ark.data"
private val BASE_URI: Uri = Uri.parse("content://$AUTHORITY")

private fun tableUri(table: String): Uri = BASE_URI.buildUpon().appendPath(table).build()
private fun itemUri(table: String, id: String): Uri =
    BASE_URI.buildUpon().appendPath(table).appendPath(id).build()

@Singleton
class ArkDataRepository @Inject constructor(
    @ApplicationContext private val context: Context,
) {
    private val contentResolver: ContentResolver = context.contentResolver
    private val json = Json { ignoreUnknownKeys = true }

    // -----------------------------------------------------------------------
    // Availability
    // -----------------------------------------------------------------------

    /** Returns true if the ark-data package is installed and its provider is reachable. */
    fun isAvailable(): Boolean = try {
        context.packageManager.getPackageInfo(ARK_DATA_PACKAGE, 0)
        true
    } catch (_: PackageManager.NameNotFoundException) {
        false
    }

    // -----------------------------------------------------------------------
    // TodoItem
    // -----------------------------------------------------------------------

    /** Observe a ContentProvider table and emit query results on IO dispatcher. */
    private fun <T> observeTable(
        table: String,
        query: () -> List<T>,
    ): Flow<List<T>> = if (!isAvailable()) {
        flowOf(emptyList())
    } else {
        callbackFlow {
            val ioScope = CoroutineScope(Dispatchers.IO + SupervisorJob())
            fun load() {
                ioScope.launch {
                    val result = query()
                    trySend(result)
                }
            }
            load()
            val observer = object : ContentObserver(Handler(Looper.getMainLooper())) {
                override fun onChange(selfChange: Boolean) { load() }
            }
            contentResolver.registerContentObserver(tableUri(table), true, observer)
            awaitClose { contentResolver.unregisterContentObserver(observer) }
        }.distinctUntilChanged()
    }

    fun getTodosFlow(): Flow<List<TodoItem>> = observeTable("todos") { queryAllTodos() }

    fun getLogbookFlow(): Flow<List<TodoItem>> = observeTable("todos") {
        queryAllTodos()
            .filter { (it.isCompleted || it.isCancelled) && !it.isTrashed }
            .sortedByDescending { it.completedAt ?: it.cancelledAt }
    }

    fun getTrashFlow(): Flow<List<TodoItem>> = observeTable("todos") {
        queryAllTodos().filter { it.isTrashed }.sortedByDescending { it.createdAt }
    }

    fun getByProjectFlow(projectId: String): Flow<List<TodoItem>> = observeTable("todos") {
        queryAllTodos()
            .filter { it.projectId == projectId && !it.isTrashed }
            .sortedBy { it.sortOrder }
    }

    private fun queryAllTodos(): List<TodoItem> {
        val cursor = contentResolver.query(tableUri("todos"), null, null, null, null)
        if (cursor == null) {
            Log.w(TAG, "queryAllTodos: cursor is null")
            return emptyList()
        }
        return cursor.use { c ->
            val result = buildList {
                while (c.moveToNext()) {
                    parseTodo(c)?.let { add(it) }
                }
            }
            Log.d(TAG, "queryAllTodos: ${result.size} todos")
            result
        }
    }

    suspend fun getAllForSync(): List<TodoItem> = withContext(Dispatchers.IO) {
        queryAllTodos()
    }

    suspend fun getTrashedIds(): List<String> = withContext(Dispatchers.IO) {
        queryAllTodos().filter { it.isTrashed }.map { it.id }
    }

    suspend fun upsert(todo: TodoItem) = withContext(Dispatchers.IO) {
        val cv = todoToValues(todo)
        try {
            val existingCursor = contentResolver.query(itemUri("todos", todo.id), null, null, null, null)
            val exists = existingCursor?.use { it.count > 0 } ?: false
            if (exists) {
                val updated = contentResolver.update(itemUri("todos", todo.id), cv, null, null)
                Log.d(TAG, "Updated todo ${todo.id}: $updated rows")
            } else {
                val uri = contentResolver.insert(tableUri("todos"), cv)
                Log.d(TAG, "Inserted todo ${todo.id}: result=$uri")
            }
        } catch (e: Exception) {
            Log.e(TAG, "upsert failed for ${todo.id}: ${e.message}", e)
        }
    }

    suspend fun upsertAll(todos: List<TodoItem>) = withContext(Dispatchers.IO) {
        todos.forEach { upsert(it) }
    }

    suspend fun getTodoById(id: String): TodoItem? = withContext(Dispatchers.IO) {
        val cursor = contentResolver.query(itemUri("todos", id), null, null, null, null)
            ?: return@withContext null
        cursor.use { if (it.moveToFirst()) parseTodo(it) else null }
    }

    suspend fun deleteById(id: String) = withContext(Dispatchers.IO) {
        contentResolver.delete(itemUri("todos", id), null, null)
    }

    /** Delete all todos via ContentProvider (iterates over all IDs). */
    suspend fun deleteAll() = withContext(Dispatchers.IO) {
        queryAllTodos().forEach { contentResolver.delete(itemUri("todos", it.id), null, null) }
    }

    /** Delete all checklist items via ContentProvider. */
    suspend fun deleteAllChecklistItems() = withContext(Dispatchers.IO) {
        queryAllChecklistItems().forEach { contentResolver.delete(itemUri("checklist_items", it.id), null, null) }
    }

    /** Delete all tag cross-refs via ContentProvider. */
    suspend fun deleteAllTagRefs() = withContext(Dispatchers.IO) {
        queryAllCrossRefs().forEach { ref ->
            contentResolver.delete(itemUri("todo_tag_cross_ref", "${ref.todoId}:${ref.tagId}"), null, null)
        }
    }

    suspend fun deleteTrashed() = withContext(Dispatchers.IO) {
        val trashedIds = getTrashedIds()
        trashedIds.forEach { id ->
            contentResolver.delete(itemUri("todos", id), null, null)
        }
    }

    suspend fun deleteChecklistItemsByTodoIds(todoIds: List<String>) = withContext(Dispatchers.IO) {
        val all = queryAllChecklistItems()
        all.filter { it.todoItemId in todoIds }.forEach { item ->
            contentResolver.delete(itemUri("checklist_items", item.id), null, null)
        }
    }

    suspend fun deleteTagRefsByTodoIds(todoIds: List<String>) = withContext(Dispatchers.IO) {
        val all = queryAllCrossRefs()
        all.filter { it.todoId in todoIds }.forEach { ref ->
            contentResolver.delete(
                itemUri("todo_tag_cross_ref", "${ref.todoId}:${ref.tagId}"),
                null, null
            )
        }
    }

    // -----------------------------------------------------------------------
    // Project
    // -----------------------------------------------------------------------

    fun getProjectsFlow(): Flow<List<Project>> = observeTable("projects") { queryAllProjects() }

    private fun queryAllProjects(): List<Project> {
        val cursor = contentResolver.query(tableUri("projects"), null, null, null, null)
            ?: return emptyList()
        return cursor.use { c ->
            buildList {
                while (c.moveToNext()) {
                    parseProject(c)?.let { add(it) }
                }
            }
        }
    }

    suspend fun getAllProjectsForSync(): List<Project> = withContext(Dispatchers.IO) {
        queryAllProjects()
    }

    suspend fun getAllAreasForSync(): List<Area> = withContext(Dispatchers.IO) {
        queryAllAreas()
    }

    suspend fun getAllTagsForSync(): List<Tag> = withContext(Dispatchers.IO) {
        queryAllTags()
    }

    suspend fun getAllHeadingsForSync(): List<Heading> = withContext(Dispatchers.IO) {
        queryAllHeadings()
    }

    suspend fun getAllChecklistItemsForSync(): List<ChecklistItem> = withContext(Dispatchers.IO) {
        queryAllChecklistItems()
    }

    suspend fun getAllCrossRefsForSync(): List<TodoTagCrossRef> = withContext(Dispatchers.IO) {
        queryAllCrossRefs()
    }

    suspend fun getProjectById(id: String): Project? = withContext(Dispatchers.IO) {
        val cursor = contentResolver.query(itemUri("projects", id), null, null, null, null)
            ?: return@withContext null
        cursor.use { if (it.moveToFirst()) parseProject(it) else null }
    }

    suspend fun upsertProjects(projects: List<Project>) = withContext(Dispatchers.IO) {
        projects.forEach { upsertProject(it) }
    }

    /** Delete all projects via ContentProvider. */
    suspend fun deleteAllProjects() = withContext(Dispatchers.IO) {
        queryAllProjects().forEach { contentResolver.delete(itemUri("projects", it.id), null, null) }
    }

    /** Delete all areas via ContentProvider. */
    suspend fun deleteAllAreas() = withContext(Dispatchers.IO) {
        queryAllAreas().forEach { contentResolver.delete(itemUri("areas", it.id), null, null) }
    }

    /** Delete all tags via ContentProvider. */
    suspend fun deleteAllTags() = withContext(Dispatchers.IO) {
        queryAllTags().forEach { contentResolver.delete(itemUri("tags", it.id), null, null) }
    }

    /** Delete all headings via ContentProvider. */
    suspend fun deleteAllHeadings() = withContext(Dispatchers.IO) {
        queryAllHeadings().forEach { contentResolver.delete(itemUri("headings", it.id), null, null) }
    }

    private fun queryAllHeadings(): List<Heading> {
        val cursor = contentResolver.query(tableUri("headings"), null, null, null, null)
            ?: return emptyList()
        return cursor.use { c ->
            buildList {
                while (c.moveToNext()) {
                    parseHeading(c)?.let { add(it) }
                }
            }
        }
    }

    private fun parseHeading(c: android.database.Cursor): Heading? = try {
        Heading(
            id = c.getString(c.getColumnIndexOrThrow("id")),
            title = c.getString(c.getColumnIndexOrThrow("title")),
            sortOrder = c.getInt(c.getColumnIndexOrThrow("sortOrder")),
            projectId = c.getString(c.getColumnIndexOrThrow("projectId")),
        )
    } catch (e: Exception) {
        Log.e(TAG, "Failed to parse heading: ${e.message}")
        null
    }

    suspend fun upsertProject(project: Project) = withContext(Dispatchers.IO) {
        val cv = projectToValues(project)
        val existingCursor = contentResolver.query(itemUri("projects", project.id), null, null, null, null)
        val exists = existingCursor?.use { it.count > 0 } ?: false
        if (exists) {
            contentResolver.update(itemUri("projects", project.id), cv, null, null)
        } else {
            contentResolver.insert(tableUri("projects"), cv)
        }
    }

    suspend fun deleteProjectById(id: String) = withContext(Dispatchers.IO) {
        contentResolver.delete(itemUri("projects", id), null, null)
    }

    // -----------------------------------------------------------------------
    // Area
    // -----------------------------------------------------------------------

    fun getAreasFlow(): Flow<List<Area>> = observeTable("areas") { queryAllAreas() }

    private fun queryAllAreas(): List<Area> {
        val cursor = contentResolver.query(tableUri("areas"), null, null, null, null)
            ?: return emptyList()
        return cursor.use { c ->
            buildList {
                while (c.moveToNext()) {
                    parseArea(c)?.let { add(it) }
                }
            }
        }
    }

    suspend fun upsertArea(area: Area) = withContext(Dispatchers.IO) {
        val cv = areaToValues(area)
        val existingCursor = contentResolver.query(itemUri("areas", area.id), null, null, null, null)
        val exists = existingCursor?.use { it.count > 0 } ?: false
        if (exists) {
            contentResolver.update(itemUri("areas", area.id), cv, null, null)
        } else {
            contentResolver.insert(tableUri("areas"), cv)
        }
    }

    // -----------------------------------------------------------------------
    // Tag
    // -----------------------------------------------------------------------

    fun getTagsFlow(): Flow<List<Tag>> = observeTable("tags") { queryAllTags() }

    private fun queryAllTags(): List<Tag> {
        val cursor = contentResolver.query(tableUri("tags"), null, null, null, null)
            ?: return emptyList()
        return cursor.use { c ->
            buildList {
                while (c.moveToNext()) {
                    parseTag(c)?.let { add(it) }
                }
            }
        }
    }

    suspend fun upsertTag(tag: Tag) = withContext(Dispatchers.IO) {
        val cv = tagToValues(tag)
        val existingCursor = contentResolver.query(itemUri("tags", tag.id), null, null, null, null)
        val exists = existingCursor?.use { it.count > 0 } ?: false
        if (exists) {
            contentResolver.update(itemUri("tags", tag.id), cv, null, null)
        } else {
            contentResolver.insert(tableUri("tags"), cv)
        }
    }

    // -----------------------------------------------------------------------
    // Heading
    // -----------------------------------------------------------------------

    suspend fun upsertHeading(heading: Heading) = withContext(Dispatchers.IO) {
        val cv = headingToValues(heading)
        val existingCursor = contentResolver.query(itemUri("headings", heading.id), null, null, null, null)
        val exists = existingCursor?.use { it.count > 0 } ?: false
        if (exists) {
            contentResolver.update(itemUri("headings", heading.id), cv, null, null)
        } else {
            contentResolver.insert(tableUri("headings"), cv)
        }
    }

    suspend fun deleteHeadingById(id: String) = withContext(Dispatchers.IO) {
        contentResolver.delete(itemUri("headings", id), null, null)
    }

    suspend fun getAreaById(id: String): Area? = withContext(Dispatchers.IO) {
        val cursor = contentResolver.query(itemUri("areas", id), null, null, null, null)
            ?: return@withContext null
        cursor.use { if (it.moveToFirst()) parseArea(it) else null }
    }

    suspend fun deleteAreaById(id: String) = withContext(Dispatchers.IO) {
        contentResolver.delete(itemUri("areas", id), null, null)
    }

    suspend fun getTagById(id: String): Tag? = withContext(Dispatchers.IO) {
        val cursor = contentResolver.query(itemUri("tags", id), null, null, null, null)
            ?: return@withContext null
        cursor.use { if (it.moveToFirst()) parseTag(it) else null }
    }

    suspend fun deleteTagById(id: String) = withContext(Dispatchers.IO) {
        contentResolver.delete(itemUri("tags", id), null, null)
    }

    suspend fun getHeadingById(id: String): Heading? = withContext(Dispatchers.IO) {
        val cursor = contentResolver.query(itemUri("headings", id), null, null, null, null)
            ?: return@withContext null
        cursor.use { if (it.moveToFirst()) parseHeading(it) else null }
    }

    suspend fun upsertChecklistItem(item: ChecklistItem) = withContext(Dispatchers.IO) {
        val cv = checklistItemToValues(item)
        val existingCursor = contentResolver.query(itemUri("checklist_items", item.id), null, null, null, null)
        val exists = existingCursor?.use { it.count > 0 } ?: false
        if (exists) {
            contentResolver.update(itemUri("checklist_items", item.id), cv, null, null)
        } else {
            contentResolver.insert(tableUri("checklist_items"), cv)
        }
    }

    suspend fun deleteChecklistItemById(id: String) = withContext(Dispatchers.IO) {
        contentResolver.delete(itemUri("checklist_items", id), null, null)
    }

    suspend fun upsertCrossRef(ref: TodoTagCrossRef) = withContext(Dispatchers.IO) {
        val cv = ContentValues().apply {
            put("todoId", ref.todoId)
            put("tagId", ref.tagId)
        }
        val compositeId = "${ref.todoId}:${ref.tagId}"
        val existingCursor = contentResolver.query(itemUri("todo_tag_cross_ref", compositeId), null, null, null, null)
        val exists = existingCursor?.use { it.count > 0 } ?: false
        if (exists) {
            contentResolver.update(itemUri("todo_tag_cross_ref", compositeId), cv, null, null)
        } else {
            contentResolver.insert(tableUri("todo_tag_cross_ref"), cv)
        }
    }

    suspend fun deleteCrossRef(todoId: String, tagId: String) = withContext(Dispatchers.IO) {
        contentResolver.delete(itemUri("todo_tag_cross_ref", "$todoId:$tagId"), null, null)
    }

    /** Get checklist items for a specific todo. */
    suspend fun getChecklistItemsByTodoId(todoId: String): List<ChecklistItem> = withContext(Dispatchers.IO) {
        queryAllChecklistItems().filter { it.todoItemId == todoId }
    }

    /** Get tag IDs for a specific todo. */
    suspend fun getTagIdsForTodo(todoId: String): List<String> = withContext(Dispatchers.IO) {
        queryAllCrossRefs().filter { it.todoId == todoId }.map { it.tagId }
    }

    // -----------------------------------------------------------------------
    // ChecklistItem
    // -----------------------------------------------------------------------

    private fun queryAllChecklistItems(): List<ChecklistItem> {
        val cursor = contentResolver.query(tableUri("checklist_items"), null, null, null, null)
            ?: return emptyList()
        return cursor.use { c ->
            buildList {
                while (c.moveToNext()) {
                    parseChecklistItem(c)?.let { add(it) }
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // TodoTagCrossRef
    // -----------------------------------------------------------------------

    private fun queryAllCrossRefs(): List<TodoTagCrossRef> {
        val cursor = contentResolver.query(tableUri("todo_tag_cross_ref"), null, null, null, null)
            ?: return emptyList()
        return cursor.use { c ->
            buildList {
                while (c.moveToNext()) {
                    val todoId = c.getString(c.getColumnIndexOrThrow("todoId"))
                    val tagId = c.getString(c.getColumnIndexOrThrow("tagId"))
                    add(TodoTagCrossRef(todoId = todoId, tagId = tagId))
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Cursor parsers
    // -----------------------------------------------------------------------

    private fun parseTodo(c: android.database.Cursor): TodoItem? = try {
        TodoItem(
            id = c.getString(c.getColumnIndexOrThrow("id")),
            title = c.getString(c.getColumnIndexOrThrow("title")),
            notes = c.getString(c.getColumnIndexOrThrow("notes")),
            priority = Priority.fromValue(c.getInt(c.getColumnIndexOrThrow("priority"))),
            scheduledDate = c.getString(c.getColumnIndexOrThrow("scheduledDate")),
            deadline = c.getString(c.getColumnIndexOrThrow("deadline")),
            reminderDate = c.getString(c.getColumnIndexOrThrow("reminderDate")),
            isToday = c.getInt(c.getColumnIndexOrThrow("isToday")) != 0,
            isEvening = c.getInt(c.getColumnIndexOrThrow("isEvening")) != 0,
            isSomeday = c.getInt(c.getColumnIndexOrThrow("isSomeday")) != 0,
            isCompleted = c.getInt(c.getColumnIndexOrThrow("isCompleted")) != 0,
            completedAt = c.getString(c.getColumnIndexOrThrow("completedAt")),
            isCancelled = c.getInt(c.getColumnIndexOrThrow("isCancelled")) != 0,
            cancelledAt = c.getString(c.getColumnIndexOrThrow("cancelledAt")),
            isTrashed = c.getInt(c.getColumnIndexOrThrow("isTrashed")) != 0,
            sortOrder = c.getInt(c.getColumnIndexOrThrow("sortOrder")),
            headingId = c.getString(c.getColumnIndexOrThrow("headingId")),
            projectId = c.getString(c.getColumnIndexOrThrow("projectId")),
            areaId = c.getString(c.getColumnIndexOrThrow("areaId")),
            recurrenceData = c.getString(c.getColumnIndexOrThrow("recurrenceData"))?.let {
                try { json.decodeFromString(RecurrenceData.serializer(), it) } catch (_: Exception) { null }
            },
            createdAt = c.getString(c.getColumnIndexOrThrow("createdAt")),
        )
    } catch (e: Exception) {
        Log.e(TAG, "Failed to parse todo: ${e.message}")
        null
    }

    private fun parseProject(c: android.database.Cursor): Project? = try {
        Project(
            id = c.getString(c.getColumnIndexOrThrow("id")),
            title = c.getString(c.getColumnIndexOrThrow("title")),
            notes = c.getString(c.getColumnIndexOrThrow("notes")),
            status = ProjectStatus.fromValue(c.getInt(c.getColumnIndexOrThrow("status"))),
            scheduledDate = c.getString(c.getColumnIndexOrThrow("scheduledDate")),
            deadline = c.getString(c.getColumnIndexOrThrow("deadline")),
            sortOrder = c.getInt(c.getColumnIndexOrThrow("sortOrder")),
            colorTag = c.getString(c.getColumnIndexOrThrow("colorTag")),
            areaId = c.getString(c.getColumnIndexOrThrow("areaId")),
            createdAt = c.getString(c.getColumnIndexOrThrow("createdAt")),
        )
    } catch (e: Exception) {
        Log.e(TAG, "Failed to parse project: ${e.message}")
        null
    }

    private fun parseArea(c: android.database.Cursor): Area? = try {
        Area(
            id = c.getString(c.getColumnIndexOrThrow("id")),
            title = c.getString(c.getColumnIndexOrThrow("title")),
            sortOrder = c.getInt(c.getColumnIndexOrThrow("sortOrder")),
            createdAt = c.getString(c.getColumnIndexOrThrow("createdAt")),
        )
    } catch (e: Exception) {
        Log.e(TAG, "Failed to parse area: ${e.message}")
        null
    }

    private fun parseTag(c: android.database.Cursor): Tag? = try {
        Tag(
            id = c.getString(c.getColumnIndexOrThrow("id")),
            title = c.getString(c.getColumnIndexOrThrow("title")),
            color = c.getString(c.getColumnIndexOrThrow("color")),
            createdAt = c.getString(c.getColumnIndexOrThrow("createdAt")),
        )
    } catch (e: Exception) {
        Log.e(TAG, "Failed to parse tag: ${e.message}")
        null
    }

    private fun parseChecklistItem(c: android.database.Cursor): ChecklistItem? = try {
        ChecklistItem(
            id = c.getString(c.getColumnIndexOrThrow("id")),
            title = c.getString(c.getColumnIndexOrThrow("title")),
            isCompleted = c.getInt(c.getColumnIndexOrThrow("isCompleted")) != 0,
            sortOrder = c.getInt(c.getColumnIndexOrThrow("sortOrder")),
            todoItemId = c.getString(c.getColumnIndexOrThrow("todoItemId")),
        )
    } catch (e: Exception) {
        Log.e(TAG, "Failed to parse checklist item: ${e.message}")
        null
    }

    // -----------------------------------------------------------------------
    // Model -> ContentValues
    // -----------------------------------------------------------------------

    private fun todoToValues(t: TodoItem): ContentValues = ContentValues().apply {
        put("id", t.id)
        put("title", t.title)
        put("notes", t.notes)
        put("priority", t.priority.value)
        put("scheduledDate", t.scheduledDate)
        put("deadline", t.deadline)
        put("reminderDate", t.reminderDate)
        put("isToday", if (t.isToday) 1 else 0)
        put("isEvening", if (t.isEvening) 1 else 0)
        put("isSomeday", if (t.isSomeday) 1 else 0)
        put("isCompleted", if (t.isCompleted) 1 else 0)
        put("completedAt", t.completedAt)
        put("isCancelled", if (t.isCancelled) 1 else 0)
        put("cancelledAt", t.cancelledAt)
        put("isTrashed", if (t.isTrashed) 1 else 0)
        put("sortOrder", t.sortOrder)
        put("headingId", t.headingId)
        put("projectId", t.projectId)
        put("areaId", t.areaId)
        put("recurrenceData", t.recurrenceData?.let { json.encodeToString(RecurrenceData.serializer(), it) })
        put("createdAt", t.createdAt)
    }

    private fun projectToValues(p: Project): ContentValues = ContentValues().apply {
        put("id", p.id)
        put("title", p.title)
        put("notes", p.notes)
        put("status", p.status.value)
        put("scheduledDate", p.scheduledDate)
        put("deadline", p.deadline)
        put("sortOrder", p.sortOrder)
        put("colorTag", p.colorTag)
        put("areaId", p.areaId)
        put("createdAt", p.createdAt)
    }

    private fun areaToValues(a: Area): ContentValues = ContentValues().apply {
        put("id", a.id)
        put("title", a.title)
        put("sortOrder", a.sortOrder)
        put("createdAt", a.createdAt)
    }

    private fun tagToValues(t: Tag): ContentValues = ContentValues().apply {
        put("id", t.id)
        put("title", t.title)
        put("color", t.color)
        put("createdAt", t.createdAt)
    }

    private fun headingToValues(h: Heading): ContentValues = ContentValues().apply {
        put("id", h.id)
        put("title", h.title)
        put("sortOrder", h.sortOrder)
        put("projectId", h.projectId)
    }

    private fun checklistItemToValues(item: ChecklistItem): ContentValues = ContentValues().apply {
        put("id", item.id)
        put("title", item.title)
        put("isCompleted", if (item.isCompleted) 1 else 0)
        put("sortOrder", item.sortOrder)
        put("todoItemId", item.todoItemId)
    }
}
