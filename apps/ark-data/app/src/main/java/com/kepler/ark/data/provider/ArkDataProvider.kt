package com.kepler.ark.data.provider

import android.content.ContentProvider
import android.content.ContentUris
import android.content.ContentValues
import android.content.UriMatcher
import android.database.Cursor
import android.database.MatrixCursor
import android.net.Uri
import androidx.room.Room
import com.kepler.ark.data.db.ArkDatabase
import com.kepler.ark.data.model.Area
import com.kepler.ark.data.model.ChecklistItem
import com.kepler.ark.data.model.Heading
import com.kepler.ark.data.model.Note
import com.kepler.ark.data.model.Priority
import com.kepler.ark.data.model.Project
import com.kepler.ark.data.model.ProjectStatus
import com.kepler.ark.data.model.Tag
import com.kepler.ark.data.model.TodoItem
import com.kepler.ark.data.model.TodoTagCrossRef

/**
 * ContentProvider for the Kepler ecosystem shared database.
 *
 * Supports CRUD operations for todos, projects, areas, tags, headings,
 * checklist_items, todo_tag_cross_ref, and notes.
 *
 * URI scheme: content://com.kepler.ark.data/<table>[/<id>]
 */
class ArkDataProvider : ContentProvider() {

    companion object {
        const val AUTHORITY = "com.kepler.ark.data"
        val BASE_URI: Uri = Uri.parse("content://$AUTHORITY")

        // URI match codes
        private const val TODOS = 1
        private const val TODO_ID = 2
        private const val PROJECTS = 3
        private const val PROJECT_ID = 4
        private const val AREAS = 5
        private const val AREA_ID = 6
        private const val TAGS = 7
        private const val TAG_ID = 8
        private const val HEADINGS = 9
        private const val HEADING_ID = 10
        private const val CHECKLIST_ITEMS = 11
        private const val CHECKLIST_ITEM_ID = 12
        private const val TODO_TAG_CROSS_REF = 13
        private const val TODO_TAG_CROSS_REF_ID = 14
        private const val NOTES = 15
        private const val NOTE_ID = 16

        private val uriMatcher = UriMatcher(UriMatcher.NO_MATCH).apply {
            addURI(AUTHORITY, "todos", TODOS)
            addURI(AUTHORITY, "todos/*", TODO_ID)
            addURI(AUTHORITY, "projects", PROJECTS)
            addURI(AUTHORITY, "projects/*", PROJECT_ID)
            addURI(AUTHORITY, "areas", AREAS)
            addURI(AUTHORITY, "areas/*", AREA_ID)
            addURI(AUTHORITY, "tags", TAGS)
            addURI(AUTHORITY, "tags/*", TAG_ID)
            addURI(AUTHORITY, "headings", HEADINGS)
            addURI(AUTHORITY, "headings/*", HEADING_ID)
            addURI(AUTHORITY, "checklist_items", CHECKLIST_ITEMS)
            addURI(AUTHORITY, "checklist_items/*", CHECKLIST_ITEM_ID)
            addURI(AUTHORITY, "todo_tag_cross_ref", TODO_TAG_CROSS_REF)
            addURI(AUTHORITY, "todo_tag_cross_ref/*", TODO_TAG_CROSS_REF_ID)
            addURI(AUTHORITY, "notes", NOTES)
            addURI(AUTHORITY, "notes/*", NOTE_ID)
        }

        // Column names for todos
        val TODO_COLUMNS = arrayOf(
            "id", "title", "notes", "priority", "scheduledDate", "deadline", "reminderDate",
            "isToday", "isEvening", "isSomeday", "isCompleted", "completedAt",
            "isCancelled", "cancelledAt", "isTrashed", "sortOrder",
            "headingId", "projectId", "areaId", "recurrenceData", "createdAt"
        )

        // Column names for projects
        val PROJECT_COLUMNS = arrayOf(
            "id", "title", "notes", "status", "scheduledDate", "deadline",
            "sortOrder", "colorTag", "areaId", "createdAt"
        )

        // Column names for areas
        val AREA_COLUMNS = arrayOf("id", "title", "sortOrder", "createdAt")

        // Column names for tags
        val TAG_COLUMNS = arrayOf("id", "title", "color", "createdAt")

        // Column names for headings
        val HEADING_COLUMNS = arrayOf("id", "title", "sortOrder", "projectId")

        // Column names for checklist_items
        val CHECKLIST_ITEM_COLUMNS = arrayOf("id", "title", "isCompleted", "sortOrder", "todoItemId")

        // Column names for todo_tag_cross_ref
        val TODO_TAG_CROSS_REF_COLUMNS = arrayOf("todoId", "tagId")

        // Column names for notes
        val NOTE_COLUMNS = arrayOf("id", "title", "body", "createdAt", "updatedAt", "isTrashed")
    }

    private lateinit var db: ArkDatabase

    override fun onCreate(): Boolean {
        db = Room.databaseBuilder(
            context!!,
            ArkDatabase::class.java,
            "ark_data.db"
        ).build()
        // Warm up Room DB on a background thread so first query isn't slow
        Thread { db.todoDao().getAll() }.start()
        return true
    }

    override fun getType(uri: Uri): String? = when (uriMatcher.match(uri)) {
        TODOS -> "vnd.android.cursor.dir/vnd.$AUTHORITY.todos"
        TODO_ID -> "vnd.android.cursor.item/vnd.$AUTHORITY.todos"
        PROJECTS -> "vnd.android.cursor.dir/vnd.$AUTHORITY.projects"
        PROJECT_ID -> "vnd.android.cursor.item/vnd.$AUTHORITY.projects"
        AREAS -> "vnd.android.cursor.dir/vnd.$AUTHORITY.areas"
        AREA_ID -> "vnd.android.cursor.item/vnd.$AUTHORITY.areas"
        TAGS -> "vnd.android.cursor.dir/vnd.$AUTHORITY.tags"
        TAG_ID -> "vnd.android.cursor.item/vnd.$AUTHORITY.tags"
        HEADINGS -> "vnd.android.cursor.dir/vnd.$AUTHORITY.headings"
        HEADING_ID -> "vnd.android.cursor.item/vnd.$AUTHORITY.headings"
        CHECKLIST_ITEMS -> "vnd.android.cursor.dir/vnd.$AUTHORITY.checklist_items"
        CHECKLIST_ITEM_ID -> "vnd.android.cursor.item/vnd.$AUTHORITY.checklist_items"
        TODO_TAG_CROSS_REF -> "vnd.android.cursor.dir/vnd.$AUTHORITY.todo_tag_cross_ref"
        TODO_TAG_CROSS_REF_ID -> "vnd.android.cursor.item/vnd.$AUTHORITY.todo_tag_cross_ref"
        NOTES -> "vnd.android.cursor.dir/vnd.$AUTHORITY.notes"
        NOTE_ID -> "vnd.android.cursor.item/vnd.$AUTHORITY.notes"
        else -> null
    }

    // -----------------------------------------------------------------------
    // QUERY
    // -----------------------------------------------------------------------

    override fun query(
        uri: Uri,
        projection: Array<out String>?,
        selection: String?,
        selectionArgs: Array<out String>?,
        sortOrder: String?,
    ): Cursor? {
        return when (uriMatcher.match(uri)) {
        TODOS -> {
            val rows = db.todoDao().getAll()
            todosToCursor(rows)
        }
        TODO_ID -> {
            val id = uri.lastPathSegment ?: return null
            val row = db.todoDao().getById(id) ?: return MatrixCursor(TODO_COLUMNS)
            todosToCursor(listOf(row))
        }
        PROJECTS -> {
            val rows = db.projectDao().getAll()
            projectsToCursor(rows)
        }
        PROJECT_ID -> {
            val id = uri.lastPathSegment ?: return null
            val row = db.projectDao().getById(id) ?: return MatrixCursor(PROJECT_COLUMNS)
            projectsToCursor(listOf(row))
        }
        AREAS -> {
            val rows = db.areaDao().getAll()
            areasToCursor(rows)
        }
        AREA_ID -> {
            val id = uri.lastPathSegment ?: return null
            val row = db.areaDao().getById(id) ?: return MatrixCursor(AREA_COLUMNS)
            areasToCursor(listOf(row))
        }
        TAGS -> {
            val rows = db.tagDao().getAll()
            tagsToCursor(rows)
        }
        TAG_ID -> {
            val id = uri.lastPathSegment ?: return null
            val row = db.tagDao().getById(id) ?: return MatrixCursor(TAG_COLUMNS)
            tagsToCursor(listOf(row))
        }
        HEADINGS -> {
            val rows = db.headingDao().getAll()
            headingsToCursor(rows)
        }
        HEADING_ID -> {
            val id = uri.lastPathSegment ?: return null
            val row = db.headingDao().getById(id) ?: return MatrixCursor(HEADING_COLUMNS)
            headingsToCursor(listOf(row))
        }
        CHECKLIST_ITEMS -> {
            val rows = db.checklistItemDao().getAll()
            checklistItemsToCursor(rows)
        }
        CHECKLIST_ITEM_ID -> {
            val id = uri.lastPathSegment ?: return null
            val row = db.checklistItemDao().getById(id) ?: return MatrixCursor(CHECKLIST_ITEM_COLUMNS)
            checklistItemsToCursor(listOf(row))
        }
        TODO_TAG_CROSS_REF -> {
            val rows = db.todoTagCrossRefDao().getAll()
            crossRefsToCursor(rows)
        }
        TODO_TAG_CROSS_REF_ID -> {
            // ID format: "todoId:tagId"
            val segment = uri.lastPathSegment ?: return null
            val parts = segment.split(":")
            if (parts.size != 2) return MatrixCursor(TODO_TAG_CROSS_REF_COLUMNS)
            val row = db.todoTagCrossRefDao().getById(parts[0], parts[1])
                ?: return MatrixCursor(TODO_TAG_CROSS_REF_COLUMNS)
            crossRefsToCursor(listOf(row))
        }
        NOTES -> {
            val rows = db.noteDao().getAll()
            notesToCursor(rows)
        }
        NOTE_ID -> {
            val id = uri.lastPathSegment ?: return null
            val row = db.noteDao().getById(id) ?: return MatrixCursor(NOTE_COLUMNS)
            notesToCursor(listOf(row))
        }
        else -> throw IllegalArgumentException("Unknown URI: $uri")
        }
    }

    // -----------------------------------------------------------------------
    // INSERT
    // -----------------------------------------------------------------------

    override fun insert(uri: Uri, values: ContentValues?): Uri? {
        val cv = values ?: return null
        return when (uriMatcher.match(uri)) {
            TODOS, TODO_ID -> {
                val todo = cvToTodo(cv) ?: return null
                db.todoDao().insert(todo)
                context?.contentResolver?.notifyChange(uri, null)
                ContentUris.withAppendedId(BASE_URI.buildUpon().appendPath("todos").build(), 1L)
                    .buildUpon().appendPath(todo.id).build()
            }
            PROJECTS, PROJECT_ID -> {
                val project = cvToProject(cv) ?: return null
                db.projectDao().insert(project)
                context?.contentResolver?.notifyChange(uri, null)
                BASE_URI.buildUpon().appendPath("projects").appendPath(project.id).build()
            }
            AREAS, AREA_ID -> {
                val area = cvToArea(cv) ?: return null
                db.areaDao().insert(area)
                context?.contentResolver?.notifyChange(uri, null)
                BASE_URI.buildUpon().appendPath("areas").appendPath(area.id).build()
            }
            TAGS, TAG_ID -> {
                val tag = cvToTag(cv) ?: return null
                db.tagDao().insert(tag)
                context?.contentResolver?.notifyChange(uri, null)
                BASE_URI.buildUpon().appendPath("tags").appendPath(tag.id).build()
            }
            HEADINGS, HEADING_ID -> {
                val heading = cvToHeading(cv) ?: return null
                db.headingDao().insert(heading)
                context?.contentResolver?.notifyChange(uri, null)
                BASE_URI.buildUpon().appendPath("headings").appendPath(heading.id).build()
            }
            CHECKLIST_ITEMS, CHECKLIST_ITEM_ID -> {
                val item = cvToChecklistItem(cv) ?: return null
                db.checklistItemDao().insert(item)
                context?.contentResolver?.notifyChange(uri, null)
                BASE_URI.buildUpon().appendPath("checklist_items").appendPath(item.id).build()
            }
            TODO_TAG_CROSS_REF, TODO_TAG_CROSS_REF_ID -> {
                val ref = cvToCrossRef(cv) ?: return null
                db.todoTagCrossRefDao().insert(ref)
                context?.contentResolver?.notifyChange(uri, null)
                BASE_URI.buildUpon().appendPath("todo_tag_cross_ref")
                    .appendPath("${ref.todoId}:${ref.tagId}").build()
            }
            NOTES, NOTE_ID -> {
                val note = cvToNote(cv) ?: return null
                db.noteDao().insert(note)
                context?.contentResolver?.notifyChange(uri, null)
                BASE_URI.buildUpon().appendPath("notes").appendPath(note.id).build()
            }
            else -> throw IllegalArgumentException("Unknown URI: $uri")
        }
    }

    // -----------------------------------------------------------------------
    // UPDATE
    // -----------------------------------------------------------------------

    override fun update(
        uri: Uri,
        values: ContentValues?,
        selection: String?,
        selectionArgs: Array<out String>?,
    ): Int {
        val cv = values ?: return 0
        return when (uriMatcher.match(uri)) {
            TODO_ID -> {
                val id = uri.lastPathSegment ?: return 0
                val existing = db.todoDao().getById(id) ?: return 0
                val updated = applyTodoValues(existing, cv)
                val count = db.todoDao().update(updated)
                if (count > 0) context?.contentResolver?.notifyChange(uri, null)
                count
            }
            PROJECT_ID -> {
                val id = uri.lastPathSegment ?: return 0
                val existing = db.projectDao().getById(id) ?: return 0
                val updated = applyProjectValues(existing, cv)
                val count = db.projectDao().update(updated)
                if (count > 0) context?.contentResolver?.notifyChange(uri, null)
                count
            }
            AREA_ID -> {
                val id = uri.lastPathSegment ?: return 0
                val existing = db.areaDao().getById(id) ?: return 0
                val updated = applyAreaValues(existing, cv)
                val count = db.areaDao().update(updated)
                if (count > 0) context?.contentResolver?.notifyChange(uri, null)
                count
            }
            TAG_ID -> {
                val id = uri.lastPathSegment ?: return 0
                val existing = db.tagDao().getById(id) ?: return 0
                val updated = applyTagValues(existing, cv)
                val count = db.tagDao().update(updated)
                if (count > 0) context?.contentResolver?.notifyChange(uri, null)
                count
            }
            HEADING_ID -> {
                val id = uri.lastPathSegment ?: return 0
                val existing = db.headingDao().getById(id) ?: return 0
                val updated = applyHeadingValues(existing, cv)
                val count = db.headingDao().update(updated)
                if (count > 0) context?.contentResolver?.notifyChange(uri, null)
                count
            }
            CHECKLIST_ITEM_ID -> {
                val id = uri.lastPathSegment ?: return 0
                val existing = db.checklistItemDao().getById(id) ?: return 0
                val updated = applyChecklistItemValues(existing, cv)
                val count = db.checklistItemDao().update(updated)
                if (count > 0) context?.contentResolver?.notifyChange(uri, null)
                count
            }
            NOTE_ID -> {
                val id = uri.lastPathSegment ?: return 0
                val existing = db.noteDao().getById(id) ?: return 0
                val updated = applyNoteValues(existing, cv)
                val count = db.noteDao().update(updated)
                if (count > 0) context?.contentResolver?.notifyChange(uri, null)
                count
            }
            else -> throw IllegalArgumentException("Update requires item URI, got: $uri")
        }
    }

    // -----------------------------------------------------------------------
    // DELETE
    // -----------------------------------------------------------------------

    override fun delete(
        uri: Uri,
        selection: String?,
        selectionArgs: Array<out String>?,
    ): Int {
        return when (uriMatcher.match(uri)) {
        TODO_ID -> {
            val id = uri.lastPathSegment ?: return 0
            val count = db.todoDao().deleteById(id)
            if (count > 0) context?.contentResolver?.notifyChange(uri, null)
            count
        }
        PROJECT_ID -> {
            val id = uri.lastPathSegment ?: return 0
            val count = db.projectDao().deleteById(id)
            if (count > 0) context?.contentResolver?.notifyChange(uri, null)
            count
        }
        AREA_ID -> {
            val id = uri.lastPathSegment ?: return 0
            val count = db.areaDao().deleteById(id)
            if (count > 0) context?.contentResolver?.notifyChange(uri, null)
            count
        }
        TAG_ID -> {
            val id = uri.lastPathSegment ?: return 0
            val count = db.tagDao().deleteById(id)
            if (count > 0) context?.contentResolver?.notifyChange(uri, null)
            count
        }
        HEADING_ID -> {
            val id = uri.lastPathSegment ?: return 0
            val count = db.headingDao().deleteById(id)
            if (count > 0) context?.contentResolver?.notifyChange(uri, null)
            count
        }
        CHECKLIST_ITEM_ID -> {
            val id = uri.lastPathSegment ?: return 0
            val count = db.checklistItemDao().deleteById(id)
            if (count > 0) context?.contentResolver?.notifyChange(uri, null)
            count
        }
        TODO_TAG_CROSS_REF_ID -> {
            val segment = uri.lastPathSegment ?: return 0
            val parts = segment.split(":")
            if (parts.size != 2) return 0
            val count = db.todoTagCrossRefDao().deleteById(parts[0], parts[1])
            if (count > 0) context?.contentResolver?.notifyChange(uri, null)
            count
        }
        NOTE_ID -> {
            val id = uri.lastPathSegment ?: return 0
            val count = db.noteDao().deleteById(id)
            if (count > 0) context?.contentResolver?.notifyChange(uri, null)
            count
        }
        else -> throw IllegalArgumentException("Delete requires item URI or cross_ref URI, got: $uri")
        }
    }

    // -----------------------------------------------------------------------
    // Cursor helpers
    // -----------------------------------------------------------------------

    private fun todosToCursor(rows: List<TodoItem>): MatrixCursor {
        val cursor = MatrixCursor(TODO_COLUMNS)
        rows.forEach { t ->
            cursor.addRow(arrayOf(
                t.id, t.title, t.notes, t.priority.value,
                t.scheduledDate, t.deadline, t.reminderDate,
                if (t.isToday) 1 else 0, if (t.isEvening) 1 else 0,
                if (t.isSomeday) 1 else 0, if (t.isCompleted) 1 else 0,
                t.completedAt, if (t.isCancelled) 1 else 0, t.cancelledAt,
                if (t.isTrashed) 1 else 0, t.sortOrder,
                t.headingId, t.projectId, t.areaId,
                recurrenceToString(t.recurrenceData),
                t.createdAt,
            ))
        }
        return cursor
    }

    private fun projectsToCursor(rows: List<Project>): MatrixCursor {
        val cursor = MatrixCursor(PROJECT_COLUMNS)
        rows.forEach { p ->
            cursor.addRow(arrayOf(
                p.id, p.title, p.notes, p.status.value,
                p.scheduledDate, p.deadline, p.sortOrder,
                p.colorTag, p.areaId, p.createdAt,
            ))
        }
        return cursor
    }

    private fun areasToCursor(rows: List<Area>): MatrixCursor {
        val cursor = MatrixCursor(AREA_COLUMNS)
        rows.forEach { a ->
            cursor.addRow(arrayOf(a.id, a.title, a.sortOrder, a.createdAt))
        }
        return cursor
    }

    private fun tagsToCursor(rows: List<Tag>): MatrixCursor {
        val cursor = MatrixCursor(TAG_COLUMNS)
        rows.forEach { t ->
            cursor.addRow(arrayOf(t.id, t.title, t.color, t.createdAt))
        }
        return cursor
    }

    private fun headingsToCursor(rows: List<Heading>): MatrixCursor {
        val cursor = MatrixCursor(HEADING_COLUMNS)
        rows.forEach { h ->
            cursor.addRow(arrayOf(h.id, h.title, h.sortOrder, h.projectId))
        }
        return cursor
    }

    private fun checklistItemsToCursor(rows: List<ChecklistItem>): MatrixCursor {
        val cursor = MatrixCursor(CHECKLIST_ITEM_COLUMNS)
        rows.forEach { c ->
            cursor.addRow(arrayOf(
                c.id, c.title, if (c.isCompleted) 1 else 0, c.sortOrder, c.todoItemId,
            ))
        }
        return cursor
    }

    private fun crossRefsToCursor(rows: List<TodoTagCrossRef>): MatrixCursor {
        val cursor = MatrixCursor(TODO_TAG_CROSS_REF_COLUMNS)
        rows.forEach { r ->
            cursor.addRow(arrayOf(r.todoId, r.tagId))
        }
        return cursor
    }

    private fun notesToCursor(rows: List<Note>): MatrixCursor {
        val cursor = MatrixCursor(NOTE_COLUMNS)
        rows.forEach { n ->
            cursor.addRow(arrayOf(n.id, n.title, n.body, n.createdAt, n.updatedAt, n.isTrashed))
        }
        return cursor
    }

    // -----------------------------------------------------------------------
    // ContentValues -> model
    // -----------------------------------------------------------------------

    private fun cvToTodo(cv: ContentValues): TodoItem? {
        val id = cv.getAsString("id") ?: return null
        val title = cv.getAsString("title") ?: return null
        val createdAt = cv.getAsString("createdAt") ?: return null
        return TodoItem(
            id = id,
            title = title,
            notes = cv.getAsString("notes"),
            priority = Priority.fromValue(cv.getAsInteger("priority") ?: 0),
            scheduledDate = cv.getAsString("scheduledDate"),
            deadline = cv.getAsString("deadline"),
            reminderDate = cv.getAsString("reminderDate"),
            isToday = (cv.getAsInteger("isToday") ?: 0) != 0,
            isEvening = (cv.getAsInteger("isEvening") ?: 0) != 0,
            isSomeday = (cv.getAsInteger("isSomeday") ?: 0) != 0,
            isCompleted = (cv.getAsInteger("isCompleted") ?: 0) != 0,
            completedAt = cv.getAsString("completedAt"),
            isCancelled = (cv.getAsInteger("isCancelled") ?: 0) != 0,
            cancelledAt = cv.getAsString("cancelledAt"),
            isTrashed = (cv.getAsInteger("isTrashed") ?: 0) != 0,
            sortOrder = cv.getAsInteger("sortOrder") ?: 0,
            headingId = cv.getAsString("headingId"),
            projectId = cv.getAsString("projectId"),
            areaId = cv.getAsString("areaId"),
            recurrenceData = stringToRecurrence(cv.getAsString("recurrenceData")),
            createdAt = createdAt,
        )
    }

    private fun applyTodoValues(existing: TodoItem, cv: ContentValues): TodoItem = existing.copy(
        title = cv.getAsString("title") ?: existing.title,
        notes = if (cv.containsKey("notes")) cv.getAsString("notes") else existing.notes,
        priority = if (cv.containsKey("priority")) Priority.fromValue(cv.getAsInteger("priority") ?: 0) else existing.priority,
        scheduledDate = if (cv.containsKey("scheduledDate")) cv.getAsString("scheduledDate") else existing.scheduledDate,
        deadline = if (cv.containsKey("deadline")) cv.getAsString("deadline") else existing.deadline,
        reminderDate = if (cv.containsKey("reminderDate")) cv.getAsString("reminderDate") else existing.reminderDate,
        isToday = if (cv.containsKey("isToday")) (cv.getAsInteger("isToday") ?: 0) != 0 else existing.isToday,
        isEvening = if (cv.containsKey("isEvening")) (cv.getAsInteger("isEvening") ?: 0) != 0 else existing.isEvening,
        isSomeday = if (cv.containsKey("isSomeday")) (cv.getAsInteger("isSomeday") ?: 0) != 0 else existing.isSomeday,
        isCompleted = if (cv.containsKey("isCompleted")) (cv.getAsInteger("isCompleted") ?: 0) != 0 else existing.isCompleted,
        completedAt = if (cv.containsKey("completedAt")) cv.getAsString("completedAt") else existing.completedAt,
        isCancelled = if (cv.containsKey("isCancelled")) (cv.getAsInteger("isCancelled") ?: 0) != 0 else existing.isCancelled,
        cancelledAt = if (cv.containsKey("cancelledAt")) cv.getAsString("cancelledAt") else existing.cancelledAt,
        isTrashed = if (cv.containsKey("isTrashed")) (cv.getAsInteger("isTrashed") ?: 0) != 0 else existing.isTrashed,
        sortOrder = cv.getAsInteger("sortOrder") ?: existing.sortOrder,
        headingId = if (cv.containsKey("headingId")) cv.getAsString("headingId") else existing.headingId,
        projectId = if (cv.containsKey("projectId")) cv.getAsString("projectId") else existing.projectId,
        areaId = if (cv.containsKey("areaId")) cv.getAsString("areaId") else existing.areaId,
        recurrenceData = if (cv.containsKey("recurrenceData")) stringToRecurrence(cv.getAsString("recurrenceData")) else existing.recurrenceData,
    )

    private fun cvToProject(cv: ContentValues): Project? {
        val id = cv.getAsString("id") ?: return null
        val title = cv.getAsString("title") ?: return null
        val createdAt = cv.getAsString("createdAt") ?: return null
        return Project(
            id = id,
            title = title,
            notes = cv.getAsString("notes"),
            status = ProjectStatus.fromValue(cv.getAsInteger("status") ?: 0),
            scheduledDate = cv.getAsString("scheduledDate"),
            deadline = cv.getAsString("deadline"),
            sortOrder = cv.getAsInteger("sortOrder") ?: 0,
            colorTag = cv.getAsString("colorTag"),
            areaId = cv.getAsString("areaId"),
            createdAt = createdAt,
        )
    }

    private fun applyProjectValues(existing: Project, cv: ContentValues): Project = existing.copy(
        title = cv.getAsString("title") ?: existing.title,
        notes = if (cv.containsKey("notes")) cv.getAsString("notes") else existing.notes,
        status = if (cv.containsKey("status")) ProjectStatus.fromValue(cv.getAsInteger("status") ?: 0) else existing.status,
        scheduledDate = if (cv.containsKey("scheduledDate")) cv.getAsString("scheduledDate") else existing.scheduledDate,
        deadline = if (cv.containsKey("deadline")) cv.getAsString("deadline") else existing.deadline,
        sortOrder = cv.getAsInteger("sortOrder") ?: existing.sortOrder,
        colorTag = if (cv.containsKey("colorTag")) cv.getAsString("colorTag") else existing.colorTag,
        areaId = if (cv.containsKey("areaId")) cv.getAsString("areaId") else existing.areaId,
    )

    private fun cvToArea(cv: ContentValues): Area? {
        val id = cv.getAsString("id") ?: return null
        val title = cv.getAsString("title") ?: return null
        val createdAt = cv.getAsString("createdAt") ?: return null
        return Area(id = id, title = title, sortOrder = cv.getAsInteger("sortOrder") ?: 0, createdAt = createdAt)
    }

    private fun applyAreaValues(existing: Area, cv: ContentValues): Area = existing.copy(
        title = cv.getAsString("title") ?: existing.title,
        sortOrder = cv.getAsInteger("sortOrder") ?: existing.sortOrder,
    )

    private fun cvToTag(cv: ContentValues): Tag? {
        val id = cv.getAsString("id") ?: return null
        val title = cv.getAsString("title") ?: return null
        val createdAt = cv.getAsString("createdAt") ?: return null
        return Tag(id = id, title = title, color = cv.getAsString("color"), createdAt = createdAt)
    }

    private fun applyTagValues(existing: Tag, cv: ContentValues): Tag = existing.copy(
        title = cv.getAsString("title") ?: existing.title,
        color = if (cv.containsKey("color")) cv.getAsString("color") else existing.color,
    )

    private fun cvToHeading(cv: ContentValues): Heading? {
        val id = cv.getAsString("id") ?: return null
        val title = cv.getAsString("title") ?: return null
        val projectId = cv.getAsString("projectId") ?: return null
        return Heading(
            id = id, title = title,
            sortOrder = cv.getAsInteger("sortOrder") ?: 0,
            projectId = projectId,
        )
    }

    private fun applyHeadingValues(existing: Heading, cv: ContentValues): Heading = existing.copy(
        title = cv.getAsString("title") ?: existing.title,
        sortOrder = cv.getAsInteger("sortOrder") ?: existing.sortOrder,
        projectId = cv.getAsString("projectId") ?: existing.projectId,
    )

    private fun cvToChecklistItem(cv: ContentValues): ChecklistItem? {
        val id = cv.getAsString("id") ?: return null
        val title = cv.getAsString("title") ?: return null
        val todoItemId = cv.getAsString("todoItemId") ?: return null
        return ChecklistItem(
            id = id, title = title,
            isCompleted = (cv.getAsInteger("isCompleted") ?: 0) != 0,
            sortOrder = cv.getAsInteger("sortOrder") ?: 0,
            todoItemId = todoItemId,
        )
    }

    private fun applyChecklistItemValues(existing: ChecklistItem, cv: ContentValues): ChecklistItem = existing.copy(
        title = cv.getAsString("title") ?: existing.title,
        isCompleted = if (cv.containsKey("isCompleted")) (cv.getAsInteger("isCompleted") ?: 0) != 0 else existing.isCompleted,
        sortOrder = cv.getAsInteger("sortOrder") ?: existing.sortOrder,
        todoItemId = cv.getAsString("todoItemId") ?: existing.todoItemId,
    )

    private fun cvToCrossRef(cv: ContentValues): TodoTagCrossRef? {
        val todoId = cv.getAsString("todoId") ?: return null
        val tagId = cv.getAsString("tagId") ?: return null
        return TodoTagCrossRef(todoId = todoId, tagId = tagId)
    }

    private fun cvToNote(cv: ContentValues): Note? {
        val id = cv.getAsString("id") ?: return null
        val title = cv.getAsString("title") ?: return null
        val body = cv.getAsString("body") ?: return null
        val createdAt = cv.getAsString("createdAt") ?: return null
        val updatedAt = cv.getAsString("updatedAt") ?: return null
        return Note(
            id = id, title = title, body = body,
            createdAt = createdAt, updatedAt = updatedAt,
            isTrashed = cv.getAsInteger("isTrashed") ?: 0,
        )
    }

    private fun applyNoteValues(existing: Note, cv: ContentValues): Note = existing.copy(
        title = cv.getAsString("title") ?: existing.title,
        body = cv.getAsString("body") ?: existing.body,
        updatedAt = cv.getAsString("updatedAt") ?: existing.updatedAt,
        isTrashed = cv.getAsInteger("isTrashed") ?: existing.isTrashed,
    )

    // -----------------------------------------------------------------------
    // Recurrence helpers (JSON via kotlinx.serialization)
    // -----------------------------------------------------------------------

    private val jsonSerializer = kotlinx.serialization.json.Json { ignoreUnknownKeys = true }

    private fun recurrenceToString(r: com.kepler.ark.data.model.RecurrenceData?): String? =
        r?.let { jsonSerializer.encodeToString(com.kepler.ark.data.model.RecurrenceData.serializer(), it) }

    private fun stringToRecurrence(s: String?): com.kepler.ark.data.model.RecurrenceData? =
        s?.let { jsonSerializer.decodeFromString(com.kepler.ark.data.model.RecurrenceData.serializer(), it) }
}
