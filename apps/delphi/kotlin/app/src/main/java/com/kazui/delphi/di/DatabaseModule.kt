package com.kazui.delphi.di

import android.content.Context
import androidx.room.Room
import com.kazui.delphi.data.db.DelphiDatabase
import com.kazui.delphi.data.db.PendingChangeDao
import com.kazui.delphi.data.repository.ArkDataRepository
import dagger.hilt.android.qualifiers.ApplicationContext
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import java.io.File
import javax.inject.Inject
import javax.inject.Singleton

/**
 * Manages per-space Room database instances (PendingChange only).
 *
 * Todo/Project data now lives in ark-data ContentProvider, accessed via [ArkDataRepository].
 *
 * Each space gets its own SQLite file at `databases/spaces/<spaceId>/delphi.db`.
 * When the active space changes, the old DB is closed and a new one is opened.
 * A [dbGeneration] counter increments on every switch so that downstream Flow
 * collectors (ViewModels) can re-subscribe to the new DAO's reactive queries.
 */
@Singleton
class DatabaseProvider @Inject constructor(
    @ApplicationContext private val context: Context,
    val arkDataRepository: ArkDataRepository,
) {
    private var currentDb: DelphiDatabase? = null
    private var currentSpaceId: String? = null

    private val _dbGeneration = MutableStateFlow(0)
    /** Bumps on every DB switch. Collectors should `flatMapLatest` on this. */
    val dbGeneration: StateFlow<Int> = _dbGeneration.asStateFlow()

    /** Open (or re-open) the database for [spaceId]. Closes the previous DB if different. */
    @Synchronized
    fun switchTo(spaceId: String) {
        if (spaceId == currentSpaceId && currentDb?.isOpen == true) return
        closeCurrentDb()
        // getDatabasePath() doesn't support subdirectories — build the path manually
        val databasesDir = context.getDatabasePath("x").parentFile!!
        val dbFile = File(databasesDir, "spaces/$spaceId/delphi.db")
        dbFile.parentFile?.mkdirs()
        currentDb = Room.databaseBuilder(context, DelphiDatabase::class.java, dbFile.absolutePath)
            .fallbackToDestructiveMigration(dropAllTables = true)
            .build()
        currentSpaceId = spaceId
        _dbGeneration.value++
    }

    /** Close the current DB (e.g. when leaving a space). */
    @Synchronized
    fun close() {
        closeCurrentDb()
        currentSpaceId = null
        _dbGeneration.value++
    }

    private fun closeCurrentDb() {
        try {
            currentDb?.close()
        } catch (_: Exception) { }
        currentDb = null
    }

    /** Returns the current [PendingChangeDao], or throws if no DB is open. */
    fun pendingChangeDao(): PendingChangeDao = requireDb().pendingChangeDao()

    /** Whether a database is currently open. */
    val isOpen: Boolean get() = currentDb?.isOpen == true

    /** Whether the ark-data ContentProvider is available. */
    val isArkDataAvailable: Boolean get() = arkDataRepository.isAvailable()

    private fun requireDb(): DelphiDatabase =
        currentDb ?: throw IllegalStateException("No space database is open. Call switchTo(spaceId) first.")

    /** Delete the database files for a given [spaceId]. */
    fun deleteSpaceDb(spaceId: String) {
        if (spaceId == currentSpaceId) close()
        val databasesDir = context.getDatabasePath("x").parentFile!!
        val dbFile = File(databasesDir, "spaces/$spaceId/delphi.db")
        // Room creates main db + WAL + SHM files
        listOf(dbFile, File("${dbFile.path}-wal"), File("${dbFile.path}-shm")).forEach { f ->
            f.delete()
        }
        // Remove directory if empty
        dbFile.parentFile?.delete()
    }
}
