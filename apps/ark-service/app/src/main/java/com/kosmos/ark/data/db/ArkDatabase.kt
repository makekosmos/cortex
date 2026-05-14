package com.kosmos.ark.data.db

import androidx.room.Database
import androidx.room.RoomDatabase
import androidx.room.TypeConverters
import com.kosmos.ark.data.model.Area
import com.kosmos.ark.data.model.ChecklistItem
import com.kosmos.ark.data.model.Heading
import com.kosmos.ark.data.model.Note
import com.kosmos.ark.data.model.Project
import com.kosmos.ark.data.model.Tag
import com.kosmos.ark.data.model.TodoItem
import com.kosmos.ark.data.model.TodoTagCrossRef

@Database(
    entities = [
        TodoItem::class,
        ChecklistItem::class,
        Project::class,
        Area::class,
        Tag::class,
        Heading::class,
        TodoTagCrossRef::class,
        Note::class,
    ],
    version = 1,
    exportSchema = false,
)
@TypeConverters(Converters::class)
abstract class ArkDatabase : RoomDatabase() {
    abstract fun todoDao(): TodoDao
    abstract fun projectDao(): ProjectDao
    abstract fun areaDao(): AreaDao
    abstract fun tagDao(): TagDao
    abstract fun headingDao(): HeadingDao
    abstract fun checklistItemDao(): ChecklistItemDao
    abstract fun todoTagCrossRefDao(): TodoTagCrossRefDao
    abstract fun noteDao(): NoteDao
}
