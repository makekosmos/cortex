package com.kepler.ark.data.di

import android.content.Context
import androidx.room.Room
import com.kepler.ark.data.db.ArkDatabase
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
    fun provideArkDatabase(@ApplicationContext context: Context): ArkDatabase =
        Room.databaseBuilder(context, ArkDatabase::class.java, "ark_data.db")
            .build()
}
