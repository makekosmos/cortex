package com.kazui.delphi.data.db

import androidx.room.Database
import androidx.room.RoomDatabase
import androidx.room.TypeConverters
import com.kazui.delphi.data.model.Area
import com.kazui.delphi.data.model.ChecklistItem
import com.kazui.delphi.data.model.Heading
import com.kazui.delphi.data.model.PendingChange
import com.kazui.delphi.data.model.Project
import com.kazui.delphi.data.model.Tag
import com.kazui.delphi.data.model.TodoItem
import com.kazui.delphi.data.model.TodoTagCrossRef

@Database(
    entities = [
        TodoItem::class,
        ChecklistItem::class,
        Project::class,
        Area::class,
        Tag::class,
        Heading::class,
        TodoTagCrossRef::class,
        PendingChange::class,
    ],
    version = 1,
    exportSchema = false,
)
@TypeConverters(Converters::class)
abstract class DelphiDatabase : RoomDatabase() {
    abstract fun todoDao(): TodoDao
    abstract fun projectDao(): ProjectDao
    abstract fun pendingChangeDao(): PendingChangeDao
}
