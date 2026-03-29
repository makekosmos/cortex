package com.kazui.delphi.data.model

import kotlinx.serialization.Serializable

enum class Priority(val value: Int) {
    NONE(0),
    LOW(1),
    MEDIUM(2),
    HIGH(3);

    companion object {
        fun fromValue(value: Int): Priority =
            entries.first { it.value == value }
    }
}

enum class ProjectStatus(val value: Int) {
    ACTIVE(0),
    SOMEDAY(1),
    COMPLETED(2);

    companion object {
        fun fromValue(value: Int): ProjectStatus =
            entries.first { it.value == value }
    }
}

@Serializable
enum class Frequency(val value: Int) {
    DAILY(0),
    WEEKLY(1),
    MONTHLY(2),
    YEARLY(3);

    companion object {
        fun fromValue(value: Int): Frequency =
            entries.first { it.value == value }
    }
}

@Serializable
enum class RecurrenceType(val value: Int) {
    FIXED(0),
    AFTER_COMPLETION(1);

    companion object {
        fun fromValue(value: Int): RecurrenceType =
            entries.first { it.value == value }
    }
}

enum class SmartList {
    INBOX,
    TODAY,
    UPCOMING,
    ANYTIME,
    SOMEDAY,
    LOGBOOK,
    TRASH,
}
