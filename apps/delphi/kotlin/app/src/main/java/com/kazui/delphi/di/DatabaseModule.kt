package com.kazui.delphi.di

import android.content.Context
import androidx.room.Room
import com.kazui.delphi.data.db.DelphiDatabase
import com.kazui.delphi.data.db.PendingChangeDao
import com.kazui.delphi.data.db.ProjectDao
import com.kazui.delphi.data.db.TodoDao
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.android.qualifiers.ApplicationContext
import dagger.hilt.components.SingletonComponent
import javax.inject.Singleton

@Module
@InstallIn(SingletonComponent::class)
object DatabaseModule {

    @Provides
    @Singleton
    fun provideDatabase(@ApplicationContext context: Context): DelphiDatabase =
        Room.databaseBuilder(context, DelphiDatabase::class.java, "delphi.db")
            .fallbackToDestructiveMigration(dropAllTables = true)
            .build()

    @Provides
    fun provideTodoDao(db: DelphiDatabase): TodoDao = db.todoDao()

    @Provides
    fun provideProjectDao(db: DelphiDatabase): ProjectDao = db.projectDao()

    @Provides
    fun providePendingChangeDao(db: DelphiDatabase): PendingChangeDao = db.pendingChangeDao()
}
