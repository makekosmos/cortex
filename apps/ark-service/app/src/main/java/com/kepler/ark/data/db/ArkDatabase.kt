package com.kepler.ark.data.db

import androidx.room.Database
import androidx.room.RoomDatabase
import androidx.room.TypeConverters
import com.kepler.ark.data.model.Area
import com.kepler.ark.data.model.ChecklistItem
import com.kepler.ark.data.model.Heading
import com.kepler.ark.data.model.Note
import com.kepler.ark.data.model.Project
import com.kepler.ark.data.model.Tag
import com.kepler.ark.data.model.TodoItem
import com.kepler.ark.data.model.TodoTagCrossRef

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
