package com.kazui.delphi.data.db

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import com.kazui.delphi.data.model.Area
import com.kazui.delphi.data.model.Heading
import com.kazui.delphi.data.model.Project
import com.kazui.delphi.data.model.Tag
import kotlinx.coroutines.flow.Flow

@Dao
interface ProjectDao {
    @Query("SELECT * FROM projects ORDER BY sortOrder ASC")
    fun getAllProjects(): Flow<List<Project>>

    @Query("SELECT * FROM projects WHERE id = :id")
    suspend fun getProjectById(id: String): Project?

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsertProject(project: Project)

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsertProjects(projects: List<Project>)

    @Query("DELETE FROM projects WHERE id = :id")
    suspend fun deleteProjectById(id: String)

    @Query("SELECT * FROM areas ORDER BY sortOrder ASC")
    fun getAllAreas(): Flow<List<Area>>

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsertArea(area: Area)

    @Query("DELETE FROM areas WHERE id = :id")
    suspend fun deleteAreaById(id: String)

    @Query("SELECT * FROM tags ORDER BY title ASC")
    fun getAllTags(): Flow<List<Tag>>

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsertTag(tag: Tag)

    @Query("DELETE FROM tags WHERE id = :id")
    suspend fun deleteTagById(id: String)

    @Query("SELECT * FROM headings WHERE projectId = :projectId ORDER BY sortOrder ASC")
    suspend fun getHeadingsByProjectId(projectId: String): List<Heading>

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsertHeading(heading: Heading)

    @Query("DELETE FROM headings WHERE id = :id")
    suspend fun deleteHeadingById(id: String)

    @Query("SELECT * FROM projects ORDER BY sortOrder ASC")
    suspend fun getAllForSync(): List<Project>

    @Query("DELETE FROM projects")
    suspend fun deleteAllProjects()

    @Query("DELETE FROM areas")
    suspend fun deleteAllAreas()

    @Query("DELETE FROM tags")
    suspend fun deleteAllTags()

    @Query("DELETE FROM headings")
    suspend fun deleteAllHeadings()
}
