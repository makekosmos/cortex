package com.kazui.delphi.data.db

import androidx.room.Database
import androidx.room.RoomDatabase
import com.kazui.delphi.data.model.PendingChange

/**
 * Delphi's local Room database.
 *
 * Only contains [PendingChange] for the sync queue.
 * All todo/project data is now stored in the ark-data ContentProvider
 * and accessed via [com.kazui.delphi.data.repository.ArkDataRepository].
 */
@Database(
    entities = [
        PendingChange::class,
    ],
    version = 2,
    exportSchema = false,
)
abstract class DelphiDatabase : RoomDatabase() {
    abstract fun pendingChangeDao(): PendingChangeDao
}
